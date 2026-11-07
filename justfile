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
    cargo fmt --all

# DATABASE_URL must be the deployment/migration credential, not the runtime role.
migrate:
    cargo run --locked -p cli -- migrate

openapi:
    cargo run --locked -p cli -- openapi > openapi.json
    cd sdk && bun run generate
