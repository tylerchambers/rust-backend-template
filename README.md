# Rust SaaS backend template

Private modular-monolith template: Axum, SQLx/PostgreSQL, official River, email magic links, server-side sessions, tenant RLS, optional Redis and a generated TypeScript client. Licensing is undecided. This branch is for review, not an authorization to deploy.

## Start locally

Prerequisites: **Rust 1.99.1 already installed**, Bun 1.4.4, Python 3.14, rootless Podman and OpenSSL. The scripts refuse an absent Rust toolchain rather than letting rustup install it implicitly. Fixed loopback ports must be free: 55439 (PostgreSQL), 51029/58029 (SMTP/capture), 56379 (Redis), 58039 (API), 58041/58042 (revision tests). Run one checkout's acceptance suite at a time.

```sh
# Optional command runner, installed only inside this checkout:
cargo install just --version 1.59.0 --locked --root .local/tools
.local/tools/bin/just dev
# Equivalent without just:
python3 scripts/dev.py
```

`dev` starts disposable dependencies, migrates using a deployment credential, and runs the API and worker. Ctrl-C stops its processes and any dependencies it started. If dependencies were already running, it leaves them intact. Never prune containers; ownership is recorded by exact container ID in `.local/dependencies.json`.

- API: http://localhost:58039
- Captured email: http://localhost:58029
- In another terminal: `python3 scripts/seed.py` creates two sample organizations and projects, owned by `owner@example.test`. Run it before logging in as that fixture address. Reruns preserve data; conflicting identities fail rather than overwrite ownership.
- Obtain a CSRF cookie by visiting `/v1/auth/confirmation`. POST `/v1/auth/request` with `{"email":"owner@example.test"}`, exact Origin, the CSRF cookie and matching `x-csrf-token`. Open the captured link in the same browser origin, then explicitly confirm. GET never signs in.

Browser console example after opening the confirmation page:

```js
await fetch('/v1/auth/request', {
  method: 'POST',
  headers: {'content-type': 'application/json',
    'x-csrf-token': document.cookie.split('; ').find(x => x.startsWith('csrf=')).slice(5)},
  body: JSON.stringify({email: 'owner@example.test'})
});
```

Use `localhost`, not a different origin or a different cookie jar. Development capture never sends external mail. Secure `__Host-session` cookies remain enabled in development; Chromium's localhost treatment is exercised by the browser suite.

## Verify

```sh
python3 scripts/verify.py
# or .local/tools/bin/just check
```

The gate installs locked SDK dependencies into `sdk/node_modules`, and cargo-audit 0.22.2 into `.local/tools` if absent. It does not install host binaries or services. It checks formatting, dependency direction and its negative control, Clippy, real PostgreSQL/RLS/security/SMTP/Redis/River tests, process interruption, release binaries, OpenAPI drift, SDK types, native and OCI HTTP/SDK/Chromium journeys, correlated logs, live-data migration compatibility between two actual application revisions, and dependency advisories. Raw output and exit codes are retained in `docs/evidence/`; RED logs are intentional regression evidence, not final failing gates.

A fresh checkout needs network access for locked packages, digest-pinned fixture images and advisory databases. The OCI artifact is Linux x86-64 in this qualified environment. No production dependencies or personal mailboxes are used.

## Contracts and commands

- Rust DTOs and `utoipa` define `openapi.json`; `sdk/src/schema.d.ts` is generated with openapi-typescript. `sdk/src/index.ts` uses openapi-fetch.
- `just openapi` deliberately updates both generated files; `just check` rejects unexplained drift.
- `DATABASE_URL=<deployment DSN> just migrate` runs official SQLx and River migration engines. API/worker startup does **not** migrate.
- `just deps`, `just seed`, `just down` manage only checkout-owned fixtures.
- `compose.yaml` describes development dependencies; the tested Podman launcher also configures the local STARTTLS peer. `deploy/compose.yaml` is an unexecuted production wiring example, not a deployment.

See [architecture and tradeoffs](docs/adr-001.md), [operations](docs/operations.md), [contribution protocol](CONTRIBUTING.md), [acceptance](docs/acceptance.md) and [handoff](docs/evidence/handoff.md).

## Trust and scope

Mailbox control is account access, not independent MFA. One verified primary address per user; no address-change, passkeys, credential management or separate recovery path. Fresh email proofs bind sensitive membership operations to user, session, action and target. Losing the mailbox has no application recovery bypass; compromising it enables account access. Revocation and expiry remain PostgreSQL-authoritative.

The live Notion hub and linked RFCs govern, with the user's current-stable/toolchain and magic-link overrides. `docs/spec/` preserves snapshots, not completion status. Independent parent review and fresh-checkout reproduction are required before release. Hosted Forgejo Actions was disabled when inspected; a checked-in workflow is not evidence of a hosted CI run.
