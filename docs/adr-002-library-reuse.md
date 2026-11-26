# ADR 002: library-owned transport, parsing and metrics

This record describes the library decision at the time. For the current
admin flag and session-based administrative authorization, see [ADR 001](adr-001.md).

## Decision

Use maintained libraries for syntax and instrumentation, while retaining the application's transactional security records. This refactor starts from `4f297b24b2f7a5ff5d75f9a639d142588e891fc3` on `refactor/library-owned-infrastructure`. There is no deployed-data compatibility requirement, but stable session identity, credential secrecy, revocation lineage and proof/authority transactions are explicit security contracts, not incidental architecture.

| Responsibility | Owner after refactor |
|---|---|
| Mailbox grammar | `email_address 0.2.9`, behind domain `EmailAddress` |
| Identity policy | Trim, lowercase the whole address, maximum 254 bytes, ASCII unquoted mailbox; no display name or domain literal. Single-label domains remain allowed. Syntax does not prove delivery or mailbox control. |
| Cookie grammar and serialization | `cookie 0.18.2`, the same cookie engine used by the candidate session ecosystem |
| Cookie scope / ambiguity | Application constructors: Secure, Path=/, no Domain; session HttpOnly/Lax/seven days and matching removal scope; CSRF readable/Strict. Reject duplicate security-cookie names across all Cookie fields before any jar could collapse them. Malformed cookie fields fail closed. |
| HTTP instrumentation and exposition | `prometheus-client 0.25.2` counters/histograms, caller-supplied registries and OpenMetrics text encoder |
| Redis instrumentation | The same library's counters and real latency histogram; redis-rs continues to own commands and the documented atomic INCR/EXPIRE script |
| Database and River observations | SQL/pool observations registered as library gauges in each scrape's registry, never mislabeled monotonic counters |
| HTTP tracing / static response headers | `tower-http 0.7.1` TraceLayer and overriding SetResponseHeaderLayer |
| Project row decoding | Private SQLx `FromRow` type and explicit domain-ID conversion |

All versions are centralized in workspace dependencies and locked. No global metrics recorder or registry is installed. Instrument handles belong to an API state or Redis adapter and clones share only that instance's instruments. Bootstrap creates a registry for each combined scrape and passes it to database, Redis and HTTP collectors; each scrape encodes once. `/metrics/http` retains a separate HTTP-only scrape using the same instrument handles. No per-path, tenant, user, email or token labels are used; the only labels are the library's bounded histogram buckets. Database collection failure returns 503 instead of a misleading partial success or stale gauge snapshot.

Existing metric sample names remain available; HTTP and Redis durations now additionally have histogram buckets/counts and proper HELP/TYPE records. Both endpoints advertise `application/openmetrics-text; version=1.0.0; charset=utf-8`. `river_attempt_retries` is a gauge over retained attempts and can decrease after retention. The existing authentication/authorization counter names describe HTTP 401/403/429 responses; HELP explicitly includes CSRF among forbidden responses.

Tower's standard request-ID layer intentionally preserves supplied IDs, so the small UUIDv7 replacement middleware remains application-owned, outside TraceLayer. Tracing only records the generated ID and matched route template (or `unmatched`), never request headers, URI/query or bodies. Response events now carry ID/route in the structured `http` span; the correlation gate follows those fields. Static headers overwrite handler values. The Tokio deadline remains because its 503 JSON error contract is already thin composition; a Tower HTTP timeout would produce an empty response. As before, the handler deadline starts after source admission, whose database timeout is separately bounded. A paused-time test exercises the actual security and transport layers and proves timeout JSON, ID replacement, static headers and one metric observation.

Operational `/ready` and combined `/metrics` remain bootstrap-owned read-only routes outside the API security/trace middleware. They do not authenticate, emit cookies or consume authentication budgets. `/metrics/http` is an API route and includes its own completed requests on subsequent scrapes. This boundary is explicit, not an assumption that layers added earlier cover new routes.

## Narrow session exception: executable qualification

The package sources fetched from crates.io and inspected are `tower-sessions 0.14.0` and `0.15.0`, `axum-login 0.18.0`, and `tower-sessions-sqlx-store 0.15.0`. The compatible published login/store line uses **tower-sessions 0.14**, not 0.15. The store uses **SQLx 0.8.6**, while this application and River use **SQLx 0.9.0**.

The isolated spike recorded in PR #2 compiled the compatible session/login types and ran against disposable PostgreSQL. It established:

1. `axum-login 0.18.0` and `tower-sessions 0.14.0` share the same Session type.
2. Passing this application's SQLx 0.9 pool into the published store fails with **E0308**, identifying distinct SQLx 0.8 and 0.9 pool types.
3. With a spike-only SQLx 0.8 pool, the actual store persists the bearer session ID directly in the primary key. Published `postgres_store.rs:160-163` also serializes the entire record, including its ID. Merely hashing its key is insufficient to guarantee bearer secrecy.
4. A real `create → load snapshot → delete → save snapshot → load` resurrects the deleted record through the store's upsert. This is expected generic KV behavior, but violates this application's no-session-resurrection/logout contract.

These are measured compatibility/semantic blockers, not a claim that the libraries are generally insecure. Stock KV sessions also do not delete the application-owned stable session UUID, simultaneous database-authoritative idle/absolute expiry, owner-scoped enumeration/revocation, old-token revocation-only handles, or the shared proof/membership/audit transaction. A custom SessionStore could preserve these rules, but would retain the existing SQL security aggregate and add another JSON record/lifecycle. We therefore do not add that layer, a duplicate runtime pool, a fork or a River downgrade. Reconsider only when a compatible store/adapter demonstrably removes security lifecycle machinery without weakening these contracts.

Primary package references:
- https://docs.rs/tower-sessions/0.14.0/tower_sessions/
- https://docs.rs/tower-sessions/0.15.0/tower_sessions/
- https://docs.rs/axum-login/0.18.0/axum_login/
- https://docs.rs/crate/tower-sessions-sqlx-store/0.15.0/source/src/postgres_store.rs

## Narrow CSRF exception: compiles, but does not simplify this product

`axum_csrf 0.12.0` **compiled and passed an actual extractor/cookie/form-token round trip**. Its docs build warning was not treated as incompatibility. The spike explicitly configured Secure and Strict cookies and tested valid/invalid proofs, duplicate Cookie fields, the current cookie-as-header protocol, and independent replica configuration.

Observed results:
- A library-generated form authenticity token verifies with the original configuration; invalid input does not.
- Copying its issued cookie value into the header (the current SDK/browser protocol) does **not** verify: it requires a distinct form token.
- A fresh default configuration on another replica rejects the original token. Its encryption key and HMAC salt must be shared/configured for replica/restart continuity.
- Repeated identical CSRF Cookie fields still verify; our ambiguity guard is still necessary.

The concrete maintenance balance is unfavorable here: it replaces only token generation/comparison after cookie grammar has already moved to `cookie`, while adding shared cryptographic configuration and a separate client token-acquisition contract. Exact Origin, unsafe-method coverage, header/cookie uniqueness, stable error mapping and privileged proof binding still remain application-owned. We retain the narrow exact-Origin plus double-submit policy, with strict single Origin/header and unique CSRF cookie enforcement. This is not server-issued-token verification, per-session proof or a substitute for the privileged one-use proof. We are not retaining it just for protocol compatibility; the additional state/configuration has no demonstrated simplification benefit for this same-origin API. Revisit if cross-origin clients or a signed-token policy become product requirements.

Source: https://docs.rs/crate/axum_csrf/0.12.0/source/src/token.rs and `src/config.rs` (the fetched published sources, not unreleased main).

## Other consolidation and unchanged contracts

- One SQL constant owns the three identical authentication-budget statements. Each caller still selects the executor, key namespace, quota and commit boundary. Confirmation/replay budgets still commit independently of token consumption.
- Login, invitation and proof mail all use the existing Lettre message/send helper. Fragment links, SMTP error sanitization and at-least-once delivery semantics are unchanged.
- The existing role decoder is reused. Missing membership remains Forbidden; an impossible stored role is treated consistently as unavailable/corrupt storage, not an authorized role.
- No migrations, session/proof SQL, policy engine, local limiter, generic cache/KV layer, queue engine, product login flow or generated API contract changed.
