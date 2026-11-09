# Independent read-only review: library reuse

## Verdict

**No substantive correctness, security, or maintainability defect found in the reviewed change.** No blocking findings. The session and CSRF exceptions are supported by actual spike code and dependency source, rather than only the implementation handoff. Existing tests were not weakened in the inspected diff.

Reviewed commit: `e966e69c7be4b3fa9011a287b3a8111e4b80b63d`.
Base / observed origin/main: `4f297b24b2f7a5ff5d75f9a639d142588e891fc3`.
Checkout: `/home/hermes/rust-backend-template-library-reuse`.
Read the requested engineering/review skills, ADR 002 and implementation handoff. Repository remained clean. No repository file edits, dependency installation, gate regeneration, service startup, pushes, or merges. Original dirty checkout was not touched.

## Findings and evidence qualifications

### Informational — cookie rejection language is broader than the parser's contract

Sources: `docs/adr-002-library-reuse.md:12`; `crates/api/src/cookies.rs:5-18`; pinned `cookie-0.18.2/src/parse.rs:109-139`, `src/lib.rs:1432-1449`.

The application rejects non-text headers, parser errors and duplicate occurrences of the requested security-cookie name across fields. That is implemented correctly. However, “Malformed cookie fields fail closed” must be read as **fields rejected by the cookie library fail closed**, not strict RFC grammar validation of every field. The dependency trims names/values and skips empty semicolon-separated segments; it does not reject every noncanonical cookie name/value. No demonstrated authorization bypass follows: authentication still hashes the actual selected bearer, CSRF still requires exact trusted Origin and a unique matching header/cookie, and the target security name cannot repeat without rejection. This is an evidence/wording qualification, not a reason to add a handwritten strict cookie parser or block adoption.

### Informational — spike proves store semantics, not an entire middleware attack

Sources: `docs/evidence/library-reuse-spike/lib.rs.txt:12-27`, `:51-56`; `final-test.log:5-10`; `pool-mismatch.log:186-212`; installed `tower-sessions-sqlx-store-0.15.0/src/postgres_store.rs:143-166,193-208`.

The spike executes create/load/delete/save/load directly through the real PostgreSQL SessionStore, and actually asserts raw bearer persistence and resurrection. It does not execute a concurrent HTTP login/logout journey through tower-sessions middleware. The ADR appropriately describes this as stock KV semantics, not a general vulnerability in the library. The observation is sufficient to reject dropping this stock store into an application whose existing aggregate explicitly forbids resurrection; it is not proof that no custom session adapter could ever satisfy the contracts. The ADR explicitly acknowledges that alternative.

## Detailed boundary assessment

### Cookies and CSRF

- `cookies.rs:5-18` examines **all** Cookie headers, preserving duplicates rather than collapsing them through a jar. It does not return early after finding a first value; a later duplicate or parse error invalidates the result.
- Session constructors retain Secure, HttpOnly, SameSite=Lax, Path=/, no Domain and seven-day lifetime; deletion uses the same constructor with zero max-age. CSRF remains readable, Secure, Strict, Path=/.
- `api/src/lib.rs` changes single-header extraction to reject repeated Origin and x-csrf-token values. Exact Origin equality, 64-byte proof length and equality to the unique cookie remain required for unsafe methods.
- HTTP regression additions test duplicate session cookies both inside one field and across separate fields; CSRF tests cover duplicate Origin, token header and cookie fields. Existing successful authentication/logout flows remain.
- No evidence of a privilege-bypass regression or ambiguity lost by introducing the library.

### EmailAddress

Sources: `domain/src/email.rs:13-31`, `application/src/lib.rs:92-103`, `application/src/memberships.rs:141-166`, domain/infrastructure email tests and auth/invitation test diffs.

- Maintained parser owns mailbox syntax. Explicit policy owns trim, ASCII lowercasing, ASCII-only/unquoted subset, maximum byte count, no display text/domain literals.
- Both login and invitation validation run before mailbox-budget, credential, invitation or delivery publication. Source-address HTTP admission still runs before handler validation, as documented.
- Normalization is shared, and returning-login and invitation tests now exercise mixed case/outer whitespace without removing their identity/security assertions.
- Lettre compatibility corpus covers punctuation, single-label domains and local/domain label size boundaries. Rejecting quoted/Unicode mailboxes is an explicit product restriction, not silently claimed full RFC support.
- No framework/persistence type introduced into the domain boundary; the internal String representation is only constructed after validation.

### Prometheus/OpenMetrics and Redis

Sources: `api/src/metrics.rs`, `api/src/lib.rs` security/http_metrics, `bootstrap/src/lib.rs:175-199`, `infrastructure/src/lib.rs:47-83`, `infrastructure/src/redis.rs:61-90,98-149`, `tests/metrics.py`.

- Counters are registered without `_total` in family names so the library emits the expected sample suffix. Latency is now a real histogram with bounded buckets and seconds observations.
- Handles are instance-owned; cloned states/adapters share only their own metrics. No global recorder or registry was added. Per-scrape registry construction and one encode avoid duplicate EOF/exposition concatenation.
- Retained River retry attempts, queue counts, lag and pool state are gauges rather than misleading monotonic counters. Database failures propagate to a 503 before exposition rather than an apparently successful partial snapshot.
- Redis observes successful, rate-limited and unavailable completed operations once. Only `Unavailable` increments the infrastructure-error counter, preserving the distinction between expected quota rejection and infrastructure failure. Existing timeout, transport, JSON and Redis command errors still map to Unavailable; cache/limiter behavior did not change.
- The parser gate genuinely invokes the Python OpenMetrics parser on both endpoints, checks media type, histogram count/sum, cumulative buckets/+Inf and bounded labels. It is not merely checking strings for metric names.
- HTTP timing remains response-production latency, not full streamed-body transfer time; no new promise of full-body latency is made. Operational routes remain explicitly outside the API layers.

### Tower composition

Sources: `api/src/transport.rs:13-68`, router tail in `api/src/lib.rs`, `api/src/transport_tests.rs` and observability test diff.

- Last-applied identify middleware is outermost: it replaces untrusted request ID before TraceLayer creates its span and overwrites the response ID afterward.
- Static overriding-header layers surround the security middleware, including its rejection/timeout responses.
- TraceLayer uses a custom sanitized span with generated ID and MatchedPath, disables default request/failure callbacks, and emits only status/duration response fields. No raw URI, headers or body were newly logged.
- Timeout still maps to existing sanitized JSON 503. Timeout test directly composes the production security and transport helpers; it asserts status, JSON body, all policy headers, generated UUIDv7 and one request observation.
- Observability test changes only correlation field location from event to span; secrecy and correlation-join assertions remain intact.

### SQLx and transactional authority

Sources: `infrastructure/src/projects.rs:90-127,130-195`, `infrastructure/src/auth.rs` diff, `application/src/memberships.rs:141-166`, `infrastructure/src/memberships.rs:178-190`.

- FromRow replaces panic-prone Row::get with fallible decoding. Query scoping, optional/not-found handling, keyset pagination, optimistic version predicate and domain ID conversion are preserved.
- Current-role query still checks user enabled state, current session revocation, database-time absolute/idle expiry and current membership under `FOR SHARE OF m,u,s` in the same transaction as publication.
- Missing membership remains Forbidden. Invalid stored role now maps to Unavailable via the preexisting central decoder, rather than pretending corrupt storage is an ordinary denial; it cannot grant authority.
- Invitation validation precedes begin; organization lock, current-role permission check, bound proof consumption, insert, queue job, audit and commit remain in their original shared transaction.
- Proof consumption retains token hash/user/session/organization/action/target/expiry/unconsumed predicates.
- Shared budget SQL is identical to the three removed strings; callers retain their distinct executors and commit boundaries, including independent confirmation-attempt accounting.
- Session/proof SQL, migrations, infrastructure membership implementation and session regression test file are unchanged by this commit.

## Session/CSRF exception qualification

- Actual spike manifest pins axum-login 0.18.0, tower-sessions 0.14.0, SQLx store 0.15.0 and axum_csrf 0.12.0. The type-compatibility test consumes the Session through the axum-login re-export, rather than just importing both packages.
- Expected mismatch feature passes the application's SQLx 0.9 pool into the store constructor; the recorded E0308 explicitly distinguishes sqlx_core 0.8.6/0.9.0. This is a real incompatible pool boundary, not a fabricated version assertion.
- The stock store's source binds raw record.id as key and serializes the entire record; merely hashing a custom key would not remove the ID from the record payload.
- CSRF spike runs FromRequestParts, produces a response cookie/form token, then verifies a subsequent request. It asserts valid form success, invalid proof failure, cookie-value-as-proof failure, repeated-cookie acceptance and fresh-default-config failure. Final recorded run contains three passing tests, zero ignored/filtered tests.
- A fresh-default-config failure is evidence of required shared cryptographic configuration, not proof multi-replica operation is impossible. ADR states that distinction correctly.
- For this exact-origin API, adopting this CSRF crate would not remove Origin policy, ambiguity checks, unsafe-method coverage or privileged proof binding. The extra client token/configuration lifecycle has no demonstrated simplification advantage. The documented exception is reasonably narrow and not based solely on undeployed compatibility.

## Test integrity and verification

Reviewed all changed test files and verification script diffs. No assertions were removed to accommodate regressions, no skips/ignores added and no security race test replaced by a weaker test. Readiness/Redis tests adapt to the new registration API while retaining their old checks and adding histogram/type checks. Observability correlation moves to the actual structured span rather than deleting the join. Full acceptance script adds native/OCI metrics gates.

Independently parsed existing evidence: **25 gate records, all exit 0; recorded durations total 90.34 seconds; Cargo log reports 39 passing test results.** These counts corroborate the handoff, but are implementer-run evidence, not an independent full-suite rerun.

Executed these already-built test artifacts identified by the final rust-tests.log (no Cargo build or evidence writes):

- `target/debug/deps/api-e702fdee5e1c3c6d`: 2 passed (metrics isolation/histogram; paused-time transport timeout).
- `target/debug/deps/email-3a612c98de063e9d`: 1 passed (domain mailbox policy).
- `target/debug/deps/email-3432f8c9f6a8dee5`: 1 passed (Lettre recipient subset).

All had zero failed, ignored or filtered tests. Existing-artifact execution is narrower than rebuilding the reviewed commit from a fresh checkout; parent retains that gate. Did not rerun DB/Redis/browser/OCI or mutation-producing spike tests under the explicit read-only scope.

## Remaining limits / handoff

- Parent fresh-checkout acceptance and publication remain pending; this report does not claim hosted CI or deployment.
- No full RFC cookie-validator or universal mailbox-deliverability guarantee is established or needed by the refactor.
- No new skill was created: this was a bounded application of the existing review workflow, not a new non-trivial reusable procedure.
- Only file created by this review: `/home/hermes/.hermes/cache/scratch/rust-reuse-independent-review.md`.
