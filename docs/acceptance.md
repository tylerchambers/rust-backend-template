# Acceptance ledger

All statuses require executable evidence, not file existence. User magic-link overrides supersede historical WebAuthn text.

| Ticket | Requirement | Status | Original acceptance |
|---|---|---|---|
| ACL-01 | Implement permission vocabulary and policy evaluator | Not verified | Table-driven role matrix and resource policy tests pass. |
| ACL-02 | Implement organization creation use case | Not verified | Organization and owner membership commit together or rollback; invalid states rejected. |
| ACL-03 | Implement invitation and membership lifecycle | Not verified | Cross-tenant invitation abuse blocked; revocation effective immediately; concurrent acceptance safe. |
| ACL-04 | Tenant switching and authorization freshness | Not verified | Switching tenant never retains prior tenant permissions; revoked membership loses access. |
| AUTH-01 | Implement WebAuthn registration and login | Not verified | Realistic browser tests cover successful ceremony, invalid origin/challenge and replay. |
| AUTH-02 | Implement server-side sessions | Not verified | Revoked/expired sessions rejected; session fixation and CSRF tests pass; no raw tokens in logs. |
| AUTH-03 | Implement WebAuthn step-up assurance | Not verified | Privileged action denied without fresh proof; challenge cannot authorize another action or be replayed. |
| AUTH-04 | Account recovery and credential management | Not verified | Last credential removal guarded; compromised session recovery tested; account takeover paths reviewed. |
| OPS-01 | Build layered test harness | Not verified | Parallel deterministic tests and one-command local CI; no production DB required. |
| OPS-02 | Adversarial security test suite | Not verified | All negative tests reject malicious requests and prove no cross-tenant disclosure. |
| OPS-03 | Observability and production process lifecycle | Not verified | Correlated HTTP/job traces; secrets redacted; health reflects dependencies. |
| OPS-04 | Build OCI artifacts and deployment example | Not verified | Fresh deployment bootstraps successfully; rollback/runbook and supply-chain checks documented. |
| OPS-05 | Release gate and developer onboarding | Not verified | Independent agent reproduces full setup and passes all tests from fresh checkout. |
| ARC-01 | Initialize Cargo workspace and reproducible toolchain | Not verified | cargo check --workspace and just dev work on clean checkout; domain dependency graph excludes infrastructure. |
| ARC-02 | Implement composition root and error taxonomy | Not verified | Fake adapters can be injected into use cases; invalid config fails fast; errors have stable codes. |
| ARC-03 | Enforce architecture fitness functions | Not verified | Intentional boundary violation fails CI; documented architecture graph matches Cargo dependencies. |
| ARC-04 | Create ADRs and agent contribution protocol | Not verified | New agent can follow documented workflow without conflicting edits. |
| JOB-01 | Integrate River and prove transactional enqueue | Not verified | Business write and job enqueue commit/rollback atomically; failed workers recover; no custom queue machinery. |
| JOB-02 | Implement invitation delivery and retry policy | Not verified | Retry does not duplicate external effect where provider supports idempotency; terminal failure observable. |
| JOB-03 | Introduce optional Redis adapters | Not verified | API operates without Redis for noncritical paths; security limiter failure policy tested; cache keys tenant-scoped. |
| JOB-04 | Implement periodic jobs and operations | Not verified | Lease recovery, graceful shutdown and scheduled execution verified. |
| API-01 | Build Axum transport boundary | Not verified | Handlers have no SQL; malformed requests return stable errors; CSRF and origin tests pass. |
| API-02 | Define OpenAPI contract and client generation | Not verified | Generated client builds and matches server; CI fails on contract drift. |
| API-03 | Expose identity and organization endpoints | Not verified | End-to-end API tests cover happy path, unauthenticated, unauthorized and cross-tenant cases. |
| API-04 | Implement sample Project aggregate end-to-end | Not verified | Full vertical slice demonstrates approved architecture and handles concurrency and tenant isolation. |
| DB-01 | Provision PostgreSQL and migration pipeline | Not verified | Fresh and upgraded databases migrate; modified applied migration detected; queries compile offline. |
| DB-02 | Create identity and tenancy schema | Not verified | Uniqueness/FKs and lifecycle invariants enforced; repeatable fixtures. |
| DB-03 | Implement transaction-local tenant context and RLS | Not verified | Cross-tenant select/update/delete denied even for direct SQL; pooled connection reuse cannot leak tenant context. |
| DB-04 | Prototype UoW and repository transaction ergonomics | Not verified | One use case writes multiple repositories atomically; rollback and conflict tests pass; ADR records tradeoffs. |
| DB-05 | Add optimistic concurrency and migration evolution tests | Not verified | Lost update prevented; schema compatibility validated with two app revisions. |

## Evidence so far
- Official River 0.6.0 + SQLx 0.9: real Postgres transaction commit/rollback test passed.
- Nonowner NOBYPASSRLS role: direct SQL missing-context and cross-tenant SELECT/UPDATE/DELETE/INSERT denial and failed-transaction context clearing passed.
- Role matrix and Project validation/revision tests: RED then GREEN.
- Workspace compile passed (entrypoints still placeholders at this checkpoint).

## Environment deviation
First cargo invocation automatically installed the exact Rust 1.99.1 rustup toolchain because only its stable alias was installed. This was unintended and contrary to the no-host-binary-install instruction; no service installation or deployment occurred. All further dependencies are project/container scoped.
