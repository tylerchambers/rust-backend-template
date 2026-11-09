# Independent HTTP/bootstrap/SDK verification

## Verdict and frozen scope

Reviewed the final working-tree delta against `36d86b89d5d3462197ca56c93b5bd9ddc77596b5` (HEAD still equals that revision). This is a dirty, uncommitted implementation snapshot, not a reviewed new commit. Writers were reported stopped. Scope is the accepted HTTP-1/2/3 remediation, session pagination, and directly shared application/infrastructure callers—not a second whole-project audit.

**No blocking production defect found in this slice.** HTTP-1/2/3 are addressed by the actual source, generated shapes, and meaningful retained implementation regressions. Independent SDK type checking and static contract checks pass. One nonblocking regression-coverage finding remains: the session traversal test does not constrain the advertised page-size bound. This is not combined-tree or release approval. Tooling remediation remains incomplete/approval-blocked per the parent; it was neither retried nor edited.

Authority: `CONTRIBUTING.md`, `docs/adr-001.md`, `docs/acceptance.md`, `docs/thermonuclear/disposition.md`, original `review-http.json`, and `implementation-runtime.md`/`implementation-locking.md`. Historical acceptance totals are not treated as results for this tree.

## Findings

### IHTTP-1 — low, nonblocking: traversal regression does not enforce bounded pages

- **Files:** `crates/api/tests/http.rs:153-182`; production `crates/infrastructure/src/auth.rs:224-252`; public contract `crates/api/src/memberships.rs:244-254`.
- **Evidence:** The test accumulates every returned page and compares the resulting ordered IDs with all seeded live sessions. It does not assert `rows.len() <= 50`, an expected page-length sequence, or that an empty terminal page was actually reached. Removing SQL `LIMIT 50` while preserving the cursor predicate would return all live rows in the first response and an empty second response; the test's exact collected-ID assertion would still pass.
- **Consequence:** The regression strongly constrains completeness, ordering, exclusive cursor use and live/account filtering, but does not guard the bounded-read part of PERSIST-2. Current production SQL does contain `LIMIT 50`; this is a demonstrated assertion gap, not a demonstrated runtime unbounded-read bug. No mutation or fixture was run to prove a failing negative control in this review.
- **Remedy:** Assert each response has at most 50 entries and explicitly record termination; for the fixed dataset, asserting the expected page lengths is also reasonable. Keep exact full traversal and excluded-session assertions.
- **Verification:** A negative control removing only the SQL limit must fail the desired-behavior test; restore it and rerun the focused public HTTP test. Integration owner owns any authorized follow-up; reviewer made no implementation edits.

## Accepted-finding disposition

### HTTP-1 — source-derived contract: addressed

- `crates/api/src/lib.rs:255-591` and `crates/api/src/memberships.rs` now attach default JSON `ErrorBody` responses to the documented operations. `ErrorBody` remains a required string `code`, consistent with `ApiError` at `lib.rs:67-95` and the transport rejection normalization at `120-188`. A default response does not assert that every endpoint emits every domain status, and no such status enumeration was added.
- All three HTML confirmation operations now declare `text/html` string success bodies. `sdk/src/index.ts:4-6` explicitly requires `{ parseAs: "text" }`. This is the supported usage, not an automatic content-type parser: an unqualified default client GET still defaults to JSON and is not claimed fixed. `sdk/tests/contracts.integration.test.ts:17-25` uses the documented call for each page.
- `ContractSecurity` declares cookie `__Host-session`. Authenticated operations have session requirements; login initiation/confirmation and the inert HTML pages do not. Unsafe operations declare required Origin and x-csrf-token headers; their descriptions require the exact configured origin and matching current cookie. These match unchanged middleware checks at `lib.rs:122-134`. Session-cookie metadata documents authentication, not a replacement for CSRF checks. Browser fetch supplies Origin; non-browser clients must supply it and maintain cookies.
- The role schemas match the accepted parser values (`memberships.rs:103-110`) and role serialization. The proof-action enum matches `infrastructure/src/memberships.rs:74-79`. Closed transport vocabularies were not incorrectly narrowed relative to those implementations.
- Independently parsed the complete `openapi.json`: all **22 operations**, **3 HTML pages**, unsafe required-header declarations, public/authenticated classification and default ErrorBody refs passed assertions. Source annotation path multiset equals the artifact operation-path multiset. Inspected generated `sdk/src/schema.d.ts` HTML/default-error/header shapes and ran the complete SDK type check successfully.
- `crates/api/tests/contract.rs` invokes `api::openapi()`, not merely a stale JSON file. The HTTP matrix (`http.rs:73-101`) invokes the composed router and asserts both status/content type and exact error object for unauthenticated sessions and forbidden organization creation. The SDK test accesses `error?.code` without casts. These are useful production constraints, although they are not exhaustive JSON Schema response validation.
- Source/generator ownership remains Rust utoipa -> OpenAPI -> openapi-typescript. `sdk/generate.ts` and `generate-entry.ts` retain the existing isolated TypeScript compiler-API compatibility bundle. No generated artifact was edited by this reviewer. Exact regenerated byte drift remains an unresolved integration gate, not something established by the static comparison.

### HTTP-2 — multi-tab confirmation: addressed without server-policy relaxation

- `crates/api/src/lib.rs:312-319` and `memberships.rs:389-396` read the current readable `csrf` cookie in the click handler. They no longer interpolate a rendering-time CSRF token into the request. The invitation target/token and login token remain captured from the page's own fragment; a second tab cannot substitute a new invitation target merely by rotating the shared CSRF cookie.
- Server Origin equality, 64-character CSRF requirement, exact cookie/header equality and duplicate-value rejection are unchanged. `cookies.rs:4-18` still rejects ambiguous names; session cookies remain Secure/HttpOnly/Path=/ and CSRF remains Secure/SameSite=Strict. Reading the CSRF cookie does not read the HttpOnly credential.
- Fragment removal and explicit button confirmation remain. Initial HTML does not embed the login/invitation secret. The proof reveal page remains inert until its button; its own action does not submit a CSRF-bearing mutation.
- `sdk/browser.mjs:7-8,41-64` uses pages in one BrowserContext and the actual native confirmation button after another page rotates the cookie. `176-206` holds an invitation in A, signs in as its invited address in B, then clicks A and checks membership readback. No mocked fetch or synthetic successful response replaces the product path.
- The previous invitation-success assertion was moved into a captured result, not weakened: success is still 201 and the second proof use still must be 403 (`137-147`). Missing-CSRF rejection remains (`127-132`). HTTP regressions add foreign-Origin and incorrect-CSRF rejection (`http.rs:287-307`) and retain duplicate Origin/header/cookie checks, scanner behavior and the live-credential control before delayed-cookie/logout replay (`457-498`).
- Retained `browser-red.log` observes actual 403 versus expected 200 at the native button; `browser-green.log` reports the completed browser journey. These are implementation-run evidence inspected here, **not a fresh independently executed browser run**. A simultaneous cookie rotation during request dispatch can still race; the change correctly fixes the established stale-rendered-token journey without claiming transactional browser cookie snapshots.

### HTTP-3 — safe useful startup diagnostics: addressed

- `crates/bootstrap/src/lib.rs:9-40` defines a closed, data-free `Failure` enum and fixed categories. Server, worker and CLI map configuration, DB connection, runtime-role validation, dependency construction, bind/serve, worker startup/run, migration and invalid-command failures at their actual boundaries (`149-207`, `265-313`). No dependency Display/Debug, DSN, arbitrary CLI argument or SMTP credential is carried into these categories.
- All three `apps/*/src/main.rs` print the category once at the executable boundary. CLI invalid-command usage is fixed text and does not echo the argument. `runtime_role` is a validation-stage category: it can also represent failure querying role safety, not a falsely precise promise that the supplied role was conclusively unsafe.
- The server shares one immutable mail adapter between login and reauthentication. `monitored_router` extraction preserves readiness/metrics closure ownership and transport composition; it does not introduce a locator or mutable singleton. Readiness/metrics are not among the documented v1 operations, so the new v1 default-error contract is not presented as a schema for those operational endpoints.
- New subprocess tests exercise actual built entrypoints: missing/invalid configuration, secret-marker origin/DSN, DB connection failure, unsafe role, occupied listener and unknown CLI command. The source asserts unsuccessful exit, category and absence of configured secret markers; retained startup RED shows the former generic category and GREEN shows the three test results passing.
- Limits: tests use substring assertions on stderr rather than parsing every JSON event, do not assert absence from stdout, and do not separately run worker-start/run or migration failure categories. Static boundary inspection supports those paths' safe mapping, but do not describe every category as subprocess-tested. Tests requiring PostgreSQL were not rerun here. This change does not introduce a universal panic sanitizer or guarantee that every third-party library can never emit data.

## Session pagination and shared callers

- Handler `memberships.rs:256-274` parses the exclusive UUID `after` query and converts it to `SessionId`. `application/src/lib.rs:48-58,123-139` carries the cursor through `Authentication::sessions` and authenticates each request before the repository call.
- `infrastructure/src/auth.rs:224-252` keeps owner equality, nonrevoked, absolute-live and idle-live predicates and adds `id > after`, ascending unique ID order and `LIMIT 50`. No admission cap is introduced; cursor possession confers no ownership. Application documentation explicitly rejects cross-request snapshot semantics under concurrent create/revoke.
- Searched every Rust `list_sessions`/`.sessions` call. The sole HTTP caller and all three direct adapter-test call sites have the new signature. Existing adapter tests retain their original assertions, adding only `None` arguments.
- Public HTTP traversal seeds more than the old cap, filters foreign/expired/revoked/idle rows, compares exact ordered IDs, and revokes/read-backs a deliberately old session. Its page-size coverage limitation is IHTTP-1, above.
- Traced API/bootstrap construction into narrowed `ProjectUnitOfWork`, `OrganizationUnitOfWork`, `MembershipUnitOfWork` and the single `Database::tenant_work` implementation. Concrete work retains one transaction; the facade change does not independently commit subordinate operations. Membership application calls retain upfront organization/user locks, current-role policy, proof consumption, mutation, audit and commit. Invitation acceptance remains email/session/organization/token-bound in the adapter.
- Read final `identity.rs` and relevant adapter authority paths: explicit identity -> session locks, fresh live-email checks, UUID-ordered user locks and FK-compatible organization serialization are wired into the actual callers. This is integration-context inspection, not independent execution or exhaustive approval of PERSIST-1 concurrency; that separate review owns its barrier proofs and residual late-wait semantics.

## Test integrity and structure

No existing assertion was weakened in the reviewed HTTP/browser/SDK/session-test delta. New tests call production contracts, router, executables and native buttons; they do not replace behavior with permissive test-only callbacks. The in-memory Mail port in the HTTP test predates this change and is not used to fake the browser journey. Browser evidence remains the appropriate proof of native cookie behavior.

Source metadata necessarily enlarges the two API files but does not add workflow branches or generic runtime machinery. Current lengths measured from disk: API lib 710 lines; membership HTTP module 397; bootstrap 314; browser script 213. No reviewed handwritten file crosses 1,000 lines. Repeated metadata is visible and local to handlers; a general metadata framework is not warranted by this bounded change. `ProjectPage` reused for session queries is a naming imperfection, not a separate behavioral defect.

## Independent execution in this review

1. `git rev-parse HEAD`, `git status --short`, `git diff --stat 36d86b8`, untracked-file inventory and focused actual diffs: inspected the stated uncommitted snapshot, including new tests.
2. `bun run check` in `sdk`: exit **0**, executing `bun node_modules/typescript/bin/tsc --noEmit`. This includes both SDK integration-test source files and generated declarations; it does not execute HTTP tests.
3. Python JSON/source assertions: **PASS**, 22 OpenAPI operations, 3 HTML pages, all default-error/header/security checks and source-path multiset comparison. Counts were computed, not inferred from the report.
4. `git diff --check`: exit **0**; HEAD readback still the stated revision.

No full verifier, dependency launcher, database fixture, browser, migration, generator write, Rust compilation, commit or push was executed. Only this report was written. There was no attempt to bypass the reported approval block. A missing root AGENTS.md was observed; existing CONTRIBUTING.md supplied repository guidance.

## Evidence inspected, not rerun

- `docs/thermonuclear/contract-green.log`: source-derived contract test passes.
- `http-final.log`: one contract test and one composed HTTP journey pass.
- `browser-red.log`: native confirmation 403 versus expected 200; `browser-green.log`: browser journey PASS.
- `startup-red.log`: expected category failed against old generic output; `startup-green.log`: CLI and two server subprocess tests pass.
- `sdk-green.log`: both integration tests pass, including typed errors/HTML.
- Implementation runtime/locking handoffs disclose their focused checks, SQLx CLI absence, fixture teardown and remaining integration work. Their claims are not promoted to independent execution here.

## Exact reviewed paths

Primary implementation/test/artifact review:

- `apps/cli/src/main.rs`, `apps/server/src/main.rs`, `apps/worker/src/main.rs`
- `apps/cli/tests/startup.rs`, `apps/server/tests/startup.rs` (new untracked tests)
- `crates/api/src/lib.rs`, `crates/api/src/memberships.rs`, `crates/api/src/cookies.rs`, `crates/api/src/transport.rs`
- `crates/api/tests/http.rs`, `crates/api/tests/contract.rs` (new untracked contract test)
- `crates/bootstrap/src/lib.rs`
- `sdk/src/index.ts`, `sdk/src/schema.d.ts` (generated shapes through source contract), `sdk/browser.mjs`
- `sdk/tests/client.integration.test.ts`, `sdk/tests/contracts.integration.test.ts` (new untracked test)
- `sdk/package.json`, `sdk/tsconfig.json`, `sdk/generate.ts`, `sdk/generate-entry.ts`, `openapi.json`

Shared caller/context review (not a second persistence/core audit):

- `crates/application/src/lib.rs` session contract and caller; `crates/application/src/memberships.rs`; `crates/application/src/projects.rs` narrowed transaction diff
- `crates/infrastructure/src/auth.rs` actual diff/session SQL; `crates/infrastructure/src/lib.rs` composition diff; `crates/infrastructure/src/identity.rs`; `crates/infrastructure/src/memberships.rs`; `crates/infrastructure/src/projects.rs`
- `crates/infrastructure/tests/sessions.rs` signature-only diff
- Authority/report/log files named above.

## Unresolved gates and ownership

- Parent integration owner must reconcile incomplete tooling remediation and approval before claiming the accepted review round complete. This report makes no TOOL-* disposition.
- Combined-tree full verifier, regenerated OpenAPI/SDK byte drift, native/OCI journeys, migration compatibility and fresh-checkout qualification remain outstanding here. Earlier baseline and focused worker logs are not substitutes.
- No fresh Rust/HTTP/browser/startup execution was performed by this independent reviewer. SDK typing and static checks are the independently executed evidence.
- SQLx CLI preparation remains unperformed as disclosed by the implementer; this review neither installed it nor relabeled online compilation as preparation.
- IHTTP-1 is an actionable bounded-pagination regression gap; no production modification was authorized for this reviewer.
- No commit, remote publication, merge, release or deployment is asserted.
