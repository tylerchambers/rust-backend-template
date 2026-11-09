default:
    @just --list

# Full local CI. Uses only disposable dependencies recorded by this checkout.
check:
    python3 scripts/verify.py

dev:
    python3 scripts/dev.py

deps:
    python3 scripts/dependencies.py up

down:
    python3 scripts/dependencies.py down

seed:
    python3 scripts/seed.py

fmt:
    python3 scripts/lint.py fmt

lint:
    python3 scripts/lint.py check

lint-controls:
    python3 tests/lint_policy.py

# DATABASE_URL must be the deployment/migration credential, not the runtime role.
migrate:
    python3 scripts/cargo.py run --locked -p cli -- migrate

openapi:
    python3 scripts/cargo.py run --locked -p cli -- openapi > openapi.json
    cd sdk && bun run generate
