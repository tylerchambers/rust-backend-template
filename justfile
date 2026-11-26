compose := "docker compose -f compose.dev.yaml"
biome := "bun sdk/node_modules/@biomejs/biome/bin/biome check --error-on-warnings"
export CARGO_TARGET_DIR := justfile_directory() / "target"
export RUSTUP_AUTO_INSTALL := "0"
export RUSTUP_TOOLCHAIN := `awk -F '"' '/^channel =/ { print $2 }' rust-toolchain.toml`

# Prefer the pinned checkout tool, if present, for recipes and their child processes.
export PATH := `if [ -x .local/bun/node_modules/.bin/bun ] && [ "$(.local/bun/node_modules/.bin/bun --version)" = "$(cat .bun-version)" ]; then printf '%s:' "$PWD/.local/bun/node_modules/.bin"; fi; printf '%s' "$PATH"`

default:
    @just --list

# Local quality checks and unit tests. No Docker or live advisory scans.
check: _install && (_contract "check")
    cargo fmt --all --check
    cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
    {{biome}} sdk apps/frontend tests package.json
    bun run --cwd sdk check
    bun run --cwd apps/frontend check
    cargo test --locked -p domain -p application
    bun test apps/frontend/tests tests/workspace.test.mjs

_install:
    @test "$(bun --version)" = "$(cat .bun-version)" && test "$(bun -p 'require("./package.json").packageManager')" = "bun@$(cat .bun-version)" || { echo "Install Bun $(cat .bun-version); .bun-version and packageManager must agree." >&2; exit 1; }
    bun install --frozen-lockfile

# Start the local workspace. Ctrl-C stops the API and worker.
dev: _build _deps
    @echo 'Workspace: http://localhost:58039 | email: http://localhost:58029 | Ctrl-C to quit'
    env -u REDIS_URL DATABASE_URL=postgres://template_app:local-app-test@127.0.0.1:55439/template PUBLIC_ORIGIN=http://localhost:58039 BIND_ADDRESS=127.0.0.1:58039 ENVIRONMENT=development MAIL_MODE=capture SMTP_PORT=51029 bun run --parallel dev:server dev:worker

_build: _install
    bun run --filter frontend build
    cargo build --locked -p server -p worker -p cli

# Start local PostgreSQL and Mailpit, migrate, and provision the restricted runtime role.
_deps:
    {{compose}} up -d --wait --wait-timeout 60 postgres mailpit
    DATABASE_URL=postgres://postgres:local-template-test@127.0.0.1:55439/template cargo run --locked -p cli -- migrate
    {{compose}} exec -T postgres psql -U postgres -d template -v ON_ERROR_STOP=1 -c "ALTER ROLE template_app LOGIN PASSWORD 'local-app-test'"

# Stop the development services; keep the database for next time.
down:
    {{compose}} stop postgres mailpit

# Load the example organizations into this Compose database.
seed: _deps
    {{compose}} exec -T postgres psql -U postgres -d template -v ON_ERROR_STOP=1 < dev/seed.sql

# Watch frontend assets while the API is running.
frontend: _install
    bun run --filter frontend dev

# Apply safe formatting fixes.
fmt: _install
    cargo fmt --all
    {{biome}} --write sdk apps/frontend tests package.json

# Live dependency checks: CI on dependency changes and on a schedule.
audit: _install
    bun audit
    cargo install --locked --version '=0.20.2' --root .local/tools cargo-deny
    .local/tools/bin/cargo-deny --locked check --deny warnings

# DATABASE_URL must be the deployment/migration credential, not the runtime role.
migrate:
    cargo run --locked -p cli -- migrate

# Update both checked-in contract artifacts after successful generation.
openapi: _install (_contract "write")

_contract mode:
    #!/usr/bin/env bash
    set -eu
    mkdir -p "$CARGO_TARGET_DIR"
    tmp=$(mktemp -d "$CARGO_TARGET_DIR/contract.XXXXXXXX")
    trap 'rm -rf "$tmp"' EXIT
    cargo run --locked --quiet -p cli -- openapi > "$tmp/openapi.json"
    bun run --cwd sdk generate.ts "$tmp/openapi.json" "$tmp/schema.d.ts"
    if [[ {{quote(mode)}} == write ]]; then
        mv "$tmp/openapi.json" openapi.json
        mv "$tmp/schema.d.ts" sdk/src/schema.d.ts
    else
        cmp openapi.json "$tmp/openapi.json"
        cmp sdk/src/schema.d.ts "$tmp/schema.d.ts"
    fi

# Run each product journey once, against disposable services isolated from development.
integration: _build
    #!/usr/bin/env bash
    set -euo pipefail
    project="rbt-integration-$(bun -p 'crypto.randomUUID()')"
    compose=(docker compose -p "$project" -f compose.dev.yaml -f compose.integration.yaml --profile redis)
    scratch=$(mktemp -d "${TMPDIR:-/tmp}/rbt-integration.XXXXXXXX")
    export TMPDIR="$scratch" TEST_TLS_DIR="$scratch/smtp" TEST_SMTP_CERT="$scratch/smtp/cert.pem"
    server= worker=
    cleanup() {
        status=$?
        trap - EXIT
        trap '' INT TERM
        for pid in $server $worker; do kill "$pid" 2>/dev/null || true; done
        for pid in $server $worker; do wait "$pid" || true; done
        if ((status != 0)); then
            for log in "$scratch"/*.log; do [[ ! -f $log ]] || tail -n 30 "$log" >&2; done
            "${compose[@]}" logs --tail 20 smoke >&2 || true
        fi
        "${compose[@]}" down --volumes --rmi local || status=1
        rm -rf "$scratch"
        exit "$status"
    }
    trap cleanup EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM

    mkdir "$TEST_TLS_DIR"
    openssl req -x509 -newkey rsa:2048 -nodes -days 2 -subj /CN=localhost \
        -addext subjectAltName=DNS:localhost -addext basicConstraints=critical,CA:FALSE \
        -addext extendedKeyUsage=serverAuth -keyout "$TEST_TLS_DIR/key.pem" -out "$TEST_SMTP_CERT"
    chmod 755 "$TEST_TLS_DIR"
    chmod 644 "$TEST_TLS_DIR/key.pem" "$TEST_SMTP_CERT"
    "${compose[@]}" up -d --wait --wait-timeout 60 postgres mailpit redis
    pg=$("${compose[@]}" port postgres 5432)
    smtp=$("${compose[@]}" port mailpit 1025)
    mail=$("${compose[@]}" port mailpit 8025)
    redis=$("${compose[@]}" port redis 6379)
    export TEST_DATABASE_URL="postgres://postgres:local-template-test@$pg/template"
    export DATABASE_URL="postgres://template_app:local-app-test@$pg/template"
    export SMTP_PORT="${smtp##*:}" MAILPIT_URL="http://$mail" TEST_REDIS_URL="redis://$redis"
    unset REDIS_URL

    DATABASE_URL="$TEST_DATABASE_URL" target/debug/cli migrate
    "${compose[@]}" exec -T postgres psql -U postgres -d template -v ON_ERROR_STOP=1 -c "ALTER ROLE template_app LOGIN PASSWORD 'local-app-test'"
    "${compose[@]}" exec -T postgres createdb -U postgres -T template seed_test
    pg_container=$("${compose[@]}" ps -q postgres)
    TEST_POSTGRES_CONTAINER="$pg_container" bun test tests/system/seed.integration.test.mjs
    "${compose[@]}" exec -T postgres psql -U postgres -d template -v ON_ERROR_STOP=1 < dev/seed.sql
    cargo test --locked --workspace --exclude domain --exclude application

    "${compose[@]}" up -d --build smoke
    image_address=$("${compose[@]}" port smoke 8080)
    curl -fsS --max-time 3 --retry 30 --retry-all-errors --retry-delay 1 --retry-max-time 30 "http://$image_address/ready"

    export BIND_ADDRESS=127.0.0.1:58139 PUBLIC_ORIGIN=http://localhost:58139 ENVIRONMENT=development MAIL_MODE=capture
    target/debug/server >"$scratch/server.log" 2>&1 & server=$!
    target/debug/worker >"$scratch/worker.log" 2>&1 & worker=$!
    deadline=$((SECONDS + 30))
    until grep -Fq 'server.ready' "$scratch/server.log" && grep -Fq 'worker.ready' "$scratch/worker.log"; do
        kill -0 "$server" && kill -0 "$worker" || exit 1
        ((SECONDS < deadline)) || { echo 'Native processes did not start in 30 seconds' >&2; exit 1; }
        sleep 0.1
    done
    curl -fsS --max-time 3 "$PUBLIC_ORIGIN/ready"
    bun test sdk/tests
    export PLAYWRIGHT_BROWSERS_PATH="$PWD/.local/playwright"
    bun sdk/node_modules/playwright/cli.js install --only-shell chromium
    bun tests/system/browser.mjs
    bun tests/system/observability.mjs "$scratch/server.log" "$scratch/worker.log"
