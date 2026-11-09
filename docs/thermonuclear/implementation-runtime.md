# Runtime implementation handoff — PERSIST-1 follow-up verified

Base: `36d86b89d5d3462197ca56c93b5bd9ddc77596b5`, branch `refactor/thermonuclear-review`. No commit or push. PERSIST-1 was subsequently implemented and focused-tested after ownership transfer; see implementation-locking.md. This is not combined-tree approval.

## Disposition

| Finding | Status | Evidence / remaining gate |
|---|---|---|
| HTTP-1 | Implemented, focused verification passed | Source utoipa metadata now declares default JSON ErrorBody on every operation, HTML string bodies on all three confirmation pages, session-cookie security, unsafe-method Origin/CSRF headers, closed role/action schema values, and session cursor. Regenerated OpenAPI and SDK, never hand-edited generated artifacts. Contract regression RED then GREEN; actual unauthenticated/forbidden HTTP matrix and typed SDK error/HTML test pass. |
| HTTP-2 | Implemented, native Chromium verified | Login and invitation buttons read the current same-origin csrf cookie at click time rather than embedding the rendering-time value. Exact Origin, duplicate-cookie/header rejection, explicit confirmation and fragment clearing remain unchanged. Native two-tab login RED returned 403 instead of 200; GREEN includes invitation tab A, invited-email login in B, acceptance in A and membership readback. |
| HTTP-3 | Implemented, subprocess verification passed | Closed bootstrap Failure enum discards raw causes and preserves stage. All three executable boundaries print one safe category; CLI prints fixed usage for invalid commands without echoing arguments. Missing origin/database, invalid secret-bearing origin, unreachable secret-bearing DSN, unsafe runtime role, occupied listener and invalid CLI command exercised. |
| PERSIST-1 | Implemented; focused PostgreSQL/Rust verification passed; independent review pending | Upfront deterministic actor/target identity locks in the real application workflow, explicit identity/session/membership sequencing, and FK-compatible organization serialization. Production-composed RED/GREEN, cross-organization shared actors, both login orderings, expiry and independent progress: implementation-locking.md; locking-focused.log (16 passing tests), locking-clippy.log and locking-rust-check.log. |
| PERSIST-2 | Implemented, public HTTP traversal verified | Exclusive UUID `after` cursor, ascending unique ID ordering, 50-row pages, last returned ID until empty. No admission cap. Public traversal covers 106 live owned sessions, timestamp ties, foreign/expired/revoked/idle exclusions, and revocation/readback of a deliberately oldest session hidden by the prior LIMIT 100. |
| PERSIST-3 | Implemented readability; focused adapters pass | Expanded runtime SQL into multiline statements with separate bind/fetch/error chains in auth/memberships/projects. No generic query infrastructure, lock/predicate changes or new modes. All three adapters have no physical lines over 200 characters. Four checked SQLx macro query strings remain byte-identical; live/offline compilation passes. SQLx CLI preparation could not run because cargo-sqlx is not installed; no SQLx metadata was hand-edited. |

## PERSIST-1 follow-up

The original runtime worker stopped at a real reproduced deadlock because core owned the necessary contracts. The parent/user subsequently transferred all necessary application/infrastructure ownership to the locking implementer. That follow-up is now implemented and focused-verified, with exact RED/GREEN, the additional organization-FK cycle, concurrency scenarios, cleanup and residual limitations in [implementation-locking.md](implementation-locking.md).

The original failing evidence below remains historical RED. It no longer describes the current checkout. `runtime-coordination.md` records the resolved handoff. Full combined-tree verification and independent review remain parent-owned.

## Real evidence and commands

All paths below are relative to `docs/thermonuclear/`; exits are actual command results. Fixed ports belonged only to this worker while these tests ran. No complete verifier was run.

- `python3 scripts/dependencies.py up`: exit 0, `runtime-dependencies.log`. Disposable PostgreSQL/SMTP/Redis, runtime role provisioned. Rust 1.99.1 was already installed, verified before Cargo.
- `cargo test --locked -p api --test http`: RED exit 101 in `pagination-red.log`, assertion that all live sessions occur exactly once failed. GREEN exit 0 in `pagination-green.log`; latest expanded matrix/traversal is `http-final.log`.
- `cargo test --locked -p api --test contract`: RED exit 101 in `contract-red.log` (default ErrorBody absent); GREEN exit 0 in `contract-green.log`. Latest combined HTTP/contract run: exit 0, `http-final.log` (2 tests).
- `cargo test --locked -p server --test startup`: RED exit 101 in `startup-red.log`, generic startup_or_runtime_failure instead of safe category. Final `cargo test --locked -p server -p cli --test startup`: exit 0, `startup-green.log` (3 tests). The DB-dependent test uses the disposable fixture's runtime DSN and TEST_DATABASE_URL deployment credential.
- `bun browser.mjs`: RED exit 1, `browser-red.log`, actual native button response 403 != 200 after shared-cookie rotation by second tab. GREEN exit 0, `browser-green.log`, expanded login/invitation multi-tab journey. Browser uses real native server, SMTP capture and River worker. Initial browser executable absence was resolved with `bun x playwright install chromium`; output retained in `browser-install.log`.
- `cargo test --locked -p infrastructure --test proof_lock_order`: **RED exit 101**, `lock-order-red.log`. Desired-behavior test failed because proof issuance returned Unavailable. `postgres-deadlock.log` contains PostgreSQL's actual `deadlock detected` and two-process wait cycle. `crates/infrastructure/tests/proof_lock_order.rs` contains the durable reproducer; no sleeps, bounded pg_blocking_pids observation, runtime-role calls.
- `TEST_DATABASE_URL=<disposable deployment DSN> cargo test --locked -p infrastructure --test auth --test sessions --test proof_binding --test proof_limits --test owner_race --test transaction --test memberships --test projects --test invitations`: exit 0, `adapters-green.log` (12 tests). An earlier attempt with the live browser worker still running interfered with the invitation token fixture; stopped the exact owned worker before this successful rerun. Never attribute that interference to adapter correctness.
- `SQLX_OFFLINE=true cargo check --locked -p infrastructure`: exit 0, `sqlx-offline.log`.
- `DATABASE_URL=<disposable deployment DSN> SQLX_OFFLINE=false cargo check --locked -p infrastructure`: exit 0, `sqlx-online.log`.
- `cargo sqlx --version`: exited with “no such command”; no host tool installed. Online compilation exercises checked query macros but is **not** a successful cargo-sqlx prepare run.
- `cargo run --locked -p cli -- openapi > openapi.json`; `cd sdk && bun generate.ts`: exit 0. Source remains canonical.
- `bun run check`: exit 0, `sdk-types.log`. Typed errors access `.error?.code` without casts; HTML supported calls explicitly use `{parseAs: "text"}`.
- `bun test`: exit 0, `sdk-green.log` (2 tests), real native HTTP.
- `bun x biome check src/index.ts tests/client.integration.test.ts tests/contracts.integration.test.ts browser.mjs`: exit 0, `sdk-format.log`.
- Owned files only formatted with rustfmt `--config skip_children=true`. No broad workspace formatter run.
- `cargo clippy --locked -p api -p infrastructure -p bootstrap -p server -p worker -p cli --all-targets -- -D warnings`: exit 0, `runtime-clippy.log`. This compiles but does not execute the failing lock-order regression.

## Implementation details / security invariants

- Session ownership/live predicates apply on every page; cursor identity confers no authority. UUID order is independent of tied created_at values. Pagination is not a cross-request snapshot under concurrent create/revoke, explicitly documented in application API.
- Security middleware was not relaxed. HTTP regressions explicitly retain missing/incorrect CSRF, cross-origin and duplicate header/cookie rejection. Session rotation/logout lineage assertions remain.
- Source metadata uses a default error response rather than pretending every operation can return every domain error. Required request headers remain explicit generated parameters. Existing SDK integration callers now supply typed header parameters even when middleware also supplies values.
- HTML scripts still keep credentials out of request URLs, logs and initial response bodies. The CSRF cookie is the existing readable noncredential cookie; the HttpOnly session cookie is not read from JavaScript.
- Bootstrap does not retain Display/Debug dependency causes in Failure. Categories are fixed strings; endpoint/error body sanitization remains unchanged. Extracted `monitored_router` separates observability route composition and shares a single immutable mail adapter between authentication and reauthentication.
- Infrastructure/projects.rs was adapted to the actual new core narrow traits: old UnitOfWork implementation removed (core moved construction to infrastructure/lib.rs), current_role now implements TenantAuthority, project_changed lives on ProjectRepository. No edits to core-owned source were made.
- No migrations, rate-limit budgets, tenant RLS policies, admission caps, worker delivery semantics or checked query contents changed.

## Owned paths changed/created

- `apps/{server,worker,cli}/src/main.rs`; new `apps/server/tests/startup.rs`, `apps/cli/tests/startup.rs`.
- `crates/bootstrap/src/lib.rs`.
- `crates/application/src/lib.rs` (session contract only).
- `crates/infrastructure/src/{auth,memberships,projects}.rs`, `crates/infrastructure/tests/sessions.rs`; new `crates/infrastructure/tests/proof_lock_order.rs`.
- `crates/api/src/{lib,memberships}.rs`, `crates/api/tests/http.rs`; new `crates/api/tests/contract.rs`.
- `sdk/src/index.ts`, `sdk/tests/client.integration.test.ts`, new `sdk/tests/contracts.integration.test.ts`, `sdk/browser.mjs`.
- Generated `openapi.json`, `sdk/src/schema.d.ts`.
- This report, runtime-coordination.md and named evidence logs in docs/thermonuclear. Pre-existing baseline evidence and other workers' dirty files are not runtime-owned.

## Remaining gates / integration owner

1. **PERSIST-1 implementation and focused GREEN completed** after ownership transfer; review implementation-locking.md and its residual limitations during independent verification. Both the retained direct regression and new application-composed matrix pass.
2. Full combined verifier, regenerated-artifact drift, native/OCI acceptance, migration compatibility and independent review belong to parent after all writers finish. Earlier baseline success does not establish current combined-tree success.
3. SQLx CLI preparation not performed (tool absent); checked strings unchanged and both compilation modes passed.
4. No commit/push/release authorization exercised.

Fixture teardown is recorded separately in runtime-cleanup.log; inspect that before reusing fixed ports.
