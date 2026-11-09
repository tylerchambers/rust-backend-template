# Independent core and persistence verification

## Verdict

**Qualified clean for the accepted CORE-1, CORE-2 and PERSIST-1/2/3 implementation scope. No current actionable correctness or structural blocker found in this delta.** This is not full-tree/release approval. Database concurrency and public HTTP execution evidence below was retained by the implementers and independently inspected, not rerun by this reviewer. Cancellation during the new identity-lock wait is source-reviewed, not behaviorally proven by a dedicated regression.

Reviewed base and current HEAD: `36d86b89d5d3462197ca56c93b5bd9ddc77596b5`; implementation is the dirty working-tree delta, including untracked identity module and concurrency tests. Review was performed after writers stopped. No fixes, blocked tooling edits, commits, pushes, DB fixtures or fixed-port resources were attempted. Only this report was created.

## Exact changed-file coverage

The following manifest was derived from `git diff --name-only 36d86b8` plus `git ls-files --others --exclude-standard` restricted to the three assigned crates. All listed deltas were inspected; new files were read in full:

- `crates/application/src/lib.rs`
- `crates/application/src/memberships.rs`
- `crates/application/src/projects.rs`
- `crates/application/tests/projects.rs`
- `crates/domain/src/lib.rs`
- `crates/domain/tests/project.rs`
- `crates/infrastructure/src/auth.rs`
- `crates/infrastructure/src/lib.rs`
- `crates/infrastructure/src/memberships.rs`
- `crates/infrastructure/src/projects.rs`
- `crates/infrastructure/tests/sessions.rs`
- `crates/infrastructure/src/identity.rs` (untracked)
- `crates/infrastructure/tests/identity_locking.rs` (untracked)
- `crates/infrastructure/tests/proof_lock_order.rs` (untracked)

Caller/contract context: `crates/api/src/memberships.rs` and `crates/api/tests/http.rs` diffs, session route/query references in `crates/api/src/lib.rs`, session operation parameters in `openapi.json`, `crates/infrastructure/tests/transaction.rs`, `crates/infrastructure/tests/rls.rs`, `CONTRIBUTING.md`, and `docs/adr-001.md`. Read the original `review-core.json`, `review-persistence.json`, disposition, and implementation-core/runtime/locking reports. Independently inspected `locking-focused.log` and `http-final.log`. Inspected the pinned River 0.6.0 `src/database.rs:21-59,98-110` for transaction cancellation ownership. Other API/SDK/tooling findings remain outside this slice.

## Finding dispositions and evidence

### CORE-1 — resolved; prior severity medium

`application/src/projects.rs:18-114` separates tenant authority and audit/commit from project, organization and membership capabilities. `application/src/memberships.rs:124-137` owns its factory/transaction surface. Project consumers cannot call membership administration methods; the two compile-fail doctests pass independently.

The application fake diff removes unsupported membership/organization panic implementations rather than substituting permissive stubs. Its existing behavioral assertions are preserved. The project job method moved into ProjectRepository; it still receives the same transaction.

`infrastructure/src/lib.rs:91-120,158-185` constructs one concrete TenantWork with one SQLx transaction for every purpose-specific factory. `infrastructure/src/projects.rs` implements the new traits, passes `&mut self.tx` to correlated River publication, and commits that transaction once. `infrastructure/src/memberships.rs:207-223` also publishes invitation jobs through `.tx(&mut self.tx)`. No separate repository commits or pool-backed nested callback was introduced. Project writes, proof consumption, role mutations, credential retirement and audit remain inside their existing atomic workflows.

**Stale handoff explicitly resolved:** implementation-core.md's statements that adapter integration is pending and infrastructure fails to compile describe an earlier snapshot. The current adapter is integrated, and this reviewer independently passed infrastructure all-targets compilation.

### CORE-2 — resolved; prior severity low

`domain/src/lib.rs:55-66` calculates the checked next version before changing either field. `domain/tests/project.rs` adds stale-version, invalid-name and exhausted-version cases asserting all aggregate fields remain unchanged. Existing test assertions were retained. The regression passed independently.

### PERSIST-1 — resolved in the accepted workflow scope; prior severity medium

Production acquisition order is explicit, not inferred from join planner behavior:

1. Each membership mutation acquires its organization's NO KEY UPDATE lock.
2. It acquires all involved user rows at UPDATE strength in ascending UUID order (`memberships.rs:134-145`). `WHERE id=ANY($1)` yields each matching row once, including actor==target.
3. Only then does it acquire/read actor session and membership authority, consume proof and mutate.
4. Retirement no longer acquires the target user late; it updates sessions using a fresh statement after the upfront lock.

Actual application callers obey this order: invite/accept/revoke include actor; remove/change_role include actor and target (`application/src/memberships.rs:166-169,204-207,223-228,251-255,285-289`). There is no actor-first SHARE-to-UPDATE upgrade or additional target acquisition after admission.

Proof issuance takes its user UPDATE lock before session SHARE and membership SHARE (`infrastructure/src/memberships.rs:84-108`). Project/discovery authority takes user SHARE before session SHARE and membership SHARE (`identity.rs:10-25`, `projects.rs:135-160`). Consequently a membership writer cannot hold a membership row while waiting for an identity held by a competing proof/reader. Opposite actor/target pairs across tenants use the same global identity order while locking only the users actually involved.

The organization lock is **NO KEY UPDATE**, not UPDATE (`memberships.rs:271-280`). This preserves mutation/last-owner serialization while allowing proof insertion's organization FK KEY SHARE. It closes the second identity→organization→identity cycle documented in the historical FK negative control. Existing project and invitation FK checks likewise do not require an incompatible organization lock. This result is bounded to current operations, not arbitrary direct SQL or hypothetical multi-organization commands.

Login (`auth.rs:135-161`) upserts the same user before inserting its session. If login wins the user lock, retirement's later READ COMMITTED statement sees the committed session. If change wins, login publishes after retirement, and that new login remains usable. Logout/authenticate/rotation do not acquire a later conflicting user lock after a session lock in the reviewed code; rotation's lineage insertion references the session already held. No additional cycle with those existing paths was identified.

**Authority freshness:** `identity::live_email` uses database `clock_timestamp()` in a fresh statement. Current-role and proof admission call it after identity/session/membership waits; organization actor validation and invitation acceptance call it after their identity/session waits. Held locks preserve revocation/disable/membership consistency; the new upfront identity wait therefore cannot admit an already expired actor using a pre-wait time predicate. Proof consumption retains exact user/session/tenant/action/target binding, one-use and expiry predicates. This is admission-time freshness, not a new promise that absolute expiry is rechecked after every later audit/storage wait or at the instant of COMMIT. That broader policy was explicitly not introduced.

**Cancellation:** the factories retain River's cancellation-safe `begin_postgres`, followed by transaction-local tenant configuration. The pinned River implementation spawns BEGIN to completion so a cancelled caller cannot return a half-started transaction to the pool. All later lock waits live inside the owned TenantWork/SQLx transaction, with no detached mutation task added by this refactor; dropping the service future drops that transaction. This is source evidence for preserved ownership, not a measured immediate-release latency or a dedicated cancel-while-lock-blocked GREEN result.

### PERSIST-2 — resolved; prior severity medium

The public chain is complete: `api/src/memberships.rs:244-274` parses the optional UUID cursor and forwards it through `application/src/lib.rs:126-139`; `infrastructure/src/auth.rs:224-247` applies owner, nonrevoked, absolute-live and idle-live predicates on every page, then exclusive `id > after`, `ORDER BY id`, `LIMIT 50`. Every public call authenticates again; the cursor is not authority. UUID uniqueness, not timestamp uniqueness, drives continuation. The application documents that concurrent creation/revocation does not provide a cross-request snapshot. No session admission cap was substituted.

The generated OpenAPI operation contains optional UUID query parameter `after` and the last-ID/until-empty contract; this was parsed directly from openapi.json. Full SDK regeneration/drift qualification remains the integration owner's responsibility.

`api/tests/http.rs:102-205` uses the actual router and database adapters, seeds owned rows with timestamp ties, makes one row deliberately older than the former LIMIT 100 window, includes foreign/expired/revoked/idle exclusions, traverses with the returned cursor, compares the complete ordered ID vector, and revokes/readbacks the formerly hidden session through public HTTP. Existing security assertions were not removed. This constrains the actual workflow, not only a repository helper.

Minor test limit, not a current defect/blocker: the traversal assertion does not separately assert each page is at most 50 rows or that an empty page was observed; an accidentally unbounded all-rows query could satisfy its final vector. The current SQL explicitly supplies the bound. Adding those assertions would strengthen future regression protection without changing scope or production behavior.

### PERSIST-3 — resolved structurally; prior severity medium

The changed security SQL in auth/memberships/projects is expanded into reviewable statements and bind/fetch/error chains. The shared identity primitive is small and fixed-purpose, not a configurable authorization/query framework. Existing live/ownership/tenant/proof predicates and binding order were inspected alongside the diff; intended lock changes are those described above. No generic repository layer or file-size explosion was introduced. Dynamic SQL still needs real PostgreSQL execution; offline compilation alone cannot validate it.

## Regression quality and retained execution evidence

- `identity_locking.rs:184-274` calls real `Memberships::change_role`; a tenant-filtered trigger pauses the actual UPDATE before retirement. The test observes DB blockers, rejects stale proof issuance, reads role/session/audit state and verifies a login queued after change can authenticate. It is not only the direct adapter sequence rewritten to follow the new contract.
- `identity_locking.rs:276-353` covers reverse actor/target order across organizations, a concurrently progressing unrelated tenant, exactly one successful authority change and exactly one audit. It does not treat Unavailable as acceptable.
- `identity_locking.rs:355-410` verifies a login committed before change is included in retirement and zero unrevoked target sessions remain.
- `identity_locking.rs:412-458` covers proof-first organization-FK ordering.
- `identity_locking.rs:34-96` waits for the actual DB expiry after an observed identity wait and requires Forbidden plus unchanged membership/session/proof/audit state.
- The matrix is bounded and sequential only for fixture DDL ownership; operations within scenarios are concurrent. Its observed queued-blocker allowance accounts for PostgreSQL waiter chains and does not relax the outcome assertions.
- The older `proof_lock_order.rs` remains a weaker direct regression (retirement success and no proof Unavailable), but the stronger production-composed test above supplies missing authority/readback assertions. It is not the sole proof of integration.
- Existing application behavioral assertions and session assertions were preserved in the reviewed diffs; session calls only gain `None` cursor arguments. No ignored tests or assertion removals were used to hide failures.
- Parsed `locking-focused.log`: **16 passing tests**, including the matrix, direct lock regression, owner race, proof binding, session lineage, RLS and transaction suites. This is retained implementer output, not an independent database rerun.
- `http-final.log` records passing contract and HTTP tests, including public pagination. Likewise retained evidence, not a new execution.

**Stale deadlock blockers:** lock-order-red.log, locking-application-red.log and locking-fk-red.log are historical failing/negative-control evidence. The current production order differs and the later focused log passes. Do not count the original handoff's reproduced deadlock as an unresolved current defect.

## Independently executed verification

Installed `1.99.1` was confirmed with `rustup toolchain list` before Cargo. No toolchain installation.

1. `cargo +1.99.1 test --offline --locked -p application -p domain` — exit 0. Application behavioral tests, domain tests and compile-fail capability/ID doctests all passed. Output specifically includes `audit_failure_publishes_neither_project_nor_job`, `memory_obeys_shared_project_contract`, and `rejected_rename_preserves_every_project_field` as OK.
2. `SQLX_OFFLINE=true cargo +1.99.1 check --offline --locked -p infrastructure --all-targets` — exit 0; finished dev profile successfully. Includes compilation of the new integration tests, not their execution.
3. `git diff --check` — exit 0, no whitespace diagnostics.
4. Git HEAD/delta manifest inspection and parsed OpenAPI cursor inspection — successful.

## Remaining gates / qualifications

- Parent owns the stable combined-tree full verifier, broader lint, generated-artifact drift, native/OCI journeys, migration compatibility and fresh-checkout qualification. This review does not approve unfinished tooling work or retry blocked edits.
- Independent DB concurrency/HTTP execution was intentionally not performed: no fixture ownership was assumed. Retained focused GREEN evidence supports the source review but is distinguished from a fresh run.
- There is no dedicated cancellation-during-new-lock-wait regression in the inspected tests. If cancellation requires executable signoff beyond preservation of the existing transaction owner, add a bounded test: block the target identity, cancel an actual Memberships command, release the barrier, prove no role/session/proof/audit publication and successful subsequent pooled work. This is a coverage limitation, not evidence of a newly introduced cancellation defect.
- SQLx CLI prepare was not run in this review; the implementation handoff reports the CLI absent and online/offline macro compilation passing. Do not report preparation as completed. Dynamic statements remain qualified by retained DB tests.
- No commit/push/release was performed. Source is still an uncommitted combined working tree.

## Issues encountered

Large combined read/diff outputs were truncated by the tool display; relevant missing original finding/disposition sections were recovered from the saved output, and production auth/identity/membership code and new tests were read directly. No execution failure blocked the focused independent checks. No production or test source was changed.
