# Independent review resolution

## Scope and outcome

Read the complete independent report at `/home/hermes/.hermes/cache/scratch/rust-backend-review.md`, all R1–R5 and numbered gaps 1–7, then compared it with the clean published `feat/saas-backend` head `d616e7d9638cb1a18510daf5ec9bf0579d198ecc`. The review described an earlier dirty checkout based on `1b266ba502ae8e207a667aa27681913264341ec6`; it was not a review of d616e7d.

All applicable implementation findings below are resolved and locally exercised. No framework, host service, production deployment, Notion edit, merge or force-push was introduced. Domain/application policy, application-owned UoW, SQL adapter ownership and tenant FORCE RLS remain intact. **Independent clean-checkout acceptance remains the parent's next gate. Hosted CI remains unverified because Forgejo Actions is disabled.** This report does not call either gate passed.

## Prioritized findings

| Finding | Applicability at d616e7d | Resolution and actual evidence |
|---|---|---|
| **R1: logout versus rotation** | Still present. Runtime-reproduced, not merely inferred from SQL. | `review-r1-red.log`: deterministic PostgreSQL test fails with `replacement survived logout`. Rotation holds the session row after changing hash and inserting the old-token handle; the test observes logout's actual `pg_blocking_pids` wait before committing rotation. Logout now resolves immutable session ID in one snapshot, then updates by ID in another statement, so an UPDATE recheck cannot lose old-token lineage. `review-r1-green.log` and final `rust-tests.log` pass. The composed API test additionally withholds the rotated cookie, proves it authenticates in a no-logout control, logs out using the old cookie, then rejects the delayed replacement cookie. |
| **R2: trusted IP and confirmation throttling** | Partly stale: current bootstrap supplies socket ConnectInfo, and middleware already limits initiation/confirmation/proof requests by peer IP; all forwarded headers are ignored. Confirmation had no mailbox budget. | Added a separate committed PostgreSQL confirmation budget derived from the token record's normalized mailbox, including failed/replayed consumption. Unknown tokens have no identifiable mailbox and are limited by socket source before token lookup. `review-r2-email-red.log` reproduces missing mailbox rejection; final `sessions.rs` passes. API tests send requests for 60 different addresses, then correctly sized invalid confirmation tokens with changing spoofed forwarded headers, observe 429, and verify initiation shares the exhausted source budget. Redis is absent in normal native/OCI acceptance, proving PostgreSQL-only operation. ADR explicitly documents 120/source and 5/mailbox per 15 minutes plus proxy trust. |
| **R3: unreachable project pagination** | Already fixed: typed HTTP `after` query and generated OpenAPI/SDK existed. | Retained the implementation; strengthened native/OCI HTTP acceptance to create 51 projects, traverse bounded pages to empty, and assert the exact unique ID set. Invalid UUID returns 400; a cursor from another tenant remains an ordering value, never authority or cross-tenant data. `native-http.log`, `oci-http.log`, generated drift and SDK gates pass. |
| **R4: revoke a chosen other session** | Already fixed: actor-scoped `revoke_session` repository/application/HTTP path existed. | Retained existing owner-scoped SQL (live caller predicate and target owner binding in the same UPDATE). `targeted_revocation_is_owner_scoped_and_preserves_other_sessions` proves A retires its second session without retiring A's first or B's session and cannot retire B's ID. Native/OCI HTTP also exercises 404 for an unavailable ID and selected-session retirement. The adjacent idle-list bug is fixed below. |
| **R5: privilege-change credentials** | Still present. | `review-r5-red.log` reproduces `pre-elevation credential remains live`. Application `Memberships::change_role` now requests credential retirement when the role actually changes, within the same UoW as role/audit publication. The adapter locks the affected user (serializing login publication), then retires sessions using a fresh statement snapshot. Other users remain signed in; no-op role assignments do not retire credentials. `review-r5-green.log`, final membership tests, concurrent owner-demotion test and native/OCI HTTP promotion/old-cookie rejection pass. Remote promotions/demotions require a new login rather than attempting to deliver another user's replacement cookie. |

## Numbered verification gaps

1. **Final races, RLS, rollback/conflict and proof binding:** ran the complete combined-tree gate. New lock-observed logout race and concurrent self-demotions leave exactly one owner; only the changed user's credentials retire. Existing live wrong-user/session/action/target/tenant, expiry and replay proof tests pass, as do River/business rollback, forced organization rollback and optimistic conflict tests. `rls_tables.rs` now exercises unscoped, own-tenant and cross-tenant SELECT visibility for all six FORCE-RLS tables (organizations, memberships, projects, invitations, audit_events, project_index), with a real nonowner role. Existing `rls.rs` additionally checks denied cross-tenant organization UPDATE/DELETE and project INSERT. New discovery-context tests prove self-only reads, denied discovery-only writes and transaction-local context reset. Global identity/security records remain protected by their explicit actor/token predicates, not falsely represented as tenant-RLS tables.
2. **Application fake ports/shared contracts:** added `application/tests/projects.rs`, a test-only snapshot/commit fake UoW. The exact `tests/project_contract.rs` suite runs against both this fake and PostgreSQL: create, rename, stale revision rejection, forbidden actor, invalid input, list and cursor behavior. A separate injected audit failure proves no project/job/audit publication. Unsupported fake capabilities panic explicitly; this is a project contract fake, not a claimed full in-memory production adapter. Domain/application still have no SQLx or HTTP-framework dependency.
3. **Organization/membership discovery:** reproduced absent GET with 405 in `review-discovery-red.log`. Added paginated `GET /v1/organizations` and `GET /v1/organizations/{organization}/members`, application read projections and generated SDK contract. Organizations are restricted to the live caller's memberships; member IDs/roles require current membership. No email disclosure or mutable active tenant. Immutable follow-up migration 10 adds SELECT-only self-discovery RLS policies and transaction-local validated-user context without widening writes. `review-discovery-green.log`, real nonowner RLS tests and native/OCI discovery/cursor/unauthorized-tenant tests pass.
4. **Idle session listing:** still present, reproduced in `review-idle-red.log`. Session listing now applies the same 30-minute idle eligibility as authentication, in addition to absolute expiry/revocation. `review-idle-green.log` and final session tests pass.
5. **Malformed origin:** already fixed by URL parsing and canonical-origin equality at d616e7d. Retained implementation and extended the config test with trailing slash and fragment cases. Existing malformed HTTPS, userinfo, path, query and insecure production cases plus the new cases pass in `rust-tests.log`.
6. **Previously assigned gates/ledger:** full local execution re-ran River SIGKILL/recovery/retries, old/current executable migration compatibility, real observability correlation, typed IDs/injected clock, toolchain checks, seed idempotency, lifecycle, OCI and browser acceptance. `acceptance.md` now references the review resolution and corrects a prior unsupported project-delete claim: the implemented sample is create/list/update, not a nonexistent delete route. `handoff.md` is explicitly historical. The user's clarification permits Rust installation; the prior unintended install is historical, not a current blocker. Parent clean-checkout qualification and hosted CI are still not asserted.
7. **Cluster-global migration role:** confirmed the documented deployment prerequisite, not a newly introduced regression: migration 002 creates `template_app` once and clean-install support requires a fresh dedicated PostgreSQL cluster. Did not rewrite an already published/applied migration or pretend second-database-in-one-cluster installation is supported. Fresh dedicated-cluster installation and separate-cluster revision qualification both pass. A multi-database provisioning split remains a deployment limitation, not a completed capability.

## Other architecture observations in the report

- **Invitation acceptance semantics:** retained the locked atomic adapter operation rather than mechanically moving revalidation outside the transaction. Added a regression in `invitations.rs`: an existing ordinary member accepts a new owner invitation, the invitation is consumed, their role stays ordinary, and replay fails. Documented that invitations join; they do not perform role changes. The original concurrent email-bound acceptance test remains.
- **Lost storage error diagnostics:** reproduced absent category logging in `review-storage-red.log`. Auth/project/membership SQLx translation now classifies failures into a closed internal category and logs only that category plus a static operation. Raw upstream messages/debug values never reach logs or HTTP. `review-storage-green.log` exercises a real closed pool and checks absence of token/DSN leakage; a unit test covers a sensitive IO cause. Public `unavailable` behavior is unchanged.
- **Existing strengths:** retained hashed one-use tokens, scanner-safe GET, CSRF/origin checks, bound proof consumption, locked last-owner policy, transaction-local tenant selection, runtime role rejection and official River transaction sharing.

## Final execution evidence

Executed `python3 scripts/verify.py` from `/home/hermes/rust-backend-template` against newly created disposable dependencies, after targeted RED/GREEN loops and formatting. Exit **0**. `results.json` contains **24 gates, every exit_code 0**. `rust-tests.log` contains **34 Cargo-reported passing results**, including the subprocess entry helper and compile-fail doctest; that count is not a claim of 34 independent business requirements.

```text
dependencies 0
seed 0
seed-idempotency 0
sdk-install 0
cargo-audit 0
bun-audit 0
format 0
architecture 0
clippy 0
rust-tests 0
dev-lifecycle 0
release 0
sdk-generate 0
sdk-types 0
native-http 0
native-sdk 0
native-browser 0
native-observability 0
revision-compatibility 0
oci-build 0
oci-http 0
oci-sdk 0
oci-browser 0
oci-observability 0
```

`git diff --check` passed. The gate removed its owned disposable containers; `podman ps` was empty afterwards. The final source fingerprint is recorded in `verified-source.json`; source changes after the complete gate are not covered unless the gate is re-run. Earlier `review-*-red.log` files intentionally fail and are not current failures. The earlier `publication.json` describes the baseline publication; final remote-ref/private/PR readback is reported with the pushed resolution commit.

## Remaining boundaries

No known unresolved implementation finding from this report. Independent fresh-checkout acceptance belongs to the parent and is still pending. Forgejo Actions is disabled, so hosted execution remains unverified. SMTP may redeliver after an ambiguous crash, production deployment/provider certification was not attempted, and multiple clean databases in one pre-provisioned PostgreSQL cluster are not advertised. None of these is silently marked accepted or repaired by local tests.
