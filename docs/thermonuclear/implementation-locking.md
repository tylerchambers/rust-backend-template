# PERSIST-1 — implemented; focused PostgreSQL and Rust verification passed

Base revision: `36d86b89d5d3462197ca56c93b5bd9ddc77596b5`. This follow-up had explicit ownership of the necessary application/infrastructure contracts after the previous runtime handoff. No commit, push, self-approval or full verifier run. Other writers' changes remain intact.

## Production change

- `MembershipRepository::lock_users(&[UserId])` is an upfront transaction contract: after organization serialization, before current-role/session/membership reads, acquire all involved identities at final UPDATE strength in ascending UUID order. The SQL selects each identity once, even when actor equals target. Change/remove include actor and target; invite/revoke/accept include actor. No all-tenant-user lock or global gate.
- `Memberships::change_role` now obeys that contract in the actual application workflow. `retire_sessions` no longer discovers/acquires the target identity late; its fresh UPDATE sees all sessions from logins that committed before the upfront lock. Login's existing user upsert still serializes subsequent session publication.
- New private `infrastructure/src/identity.rs` makes identity SHARE -> session SHARE acquisition explicit and performs a fresh live-email read after authority lock waits. `current_role` then locks membership explicitly and rechecks live identity/session state. Proof issuance acquires identity UPDATE before session/membership locks and its live check. Invitation acceptance and organization actor validation use the same small primitive, without planner-dependent multi-relation locking. Existing Forbidden versus Unauthenticated mappings are preserved.
- Organization serialization uses **FOR NO KEY UPDATE**, not FOR UPDATE. This is essential: proof INSERT has an organization FK and obtains KEY SHARE after its identity lock. FOR UPDATE would create another identity -> organization -> identity cycle. NO KEY UPDATE still conflicts with other organization mutation locks, but permits FK checks against immutable IDs.
- Existing policy, last-owner rules, proof consumption, runtime-role/RLS context, session retirement, audit writes and transaction boundaries remain. No migrations, checked SQLx query strings/metadata, auth login implementation, SDK or scripts changed by this follow-up.

## Actual RED evidence

1. The previous writer's `lock-order-red.log` and `postgres-deadlock.log` retain the original direct-transaction reproduction (Unavailable and PostgreSQL's two-process cycle).
2. Before changing production, added a **production-composed** `Memberships::change_role` test. A test-only trigger pauses the actual membership UPDATE before retirement; proof issuance is started and observed blocked via `pg_blocking_pids`; releasing the trigger barrier produces Unavailable. `locking-application-red.log`: exit **101**, test failed for that exact result. An initial SQLx 0.9 dynamic-SQL compile error was fixed before obtaining this behavioral RED (DDL identifiers/values are generated UUIDs, explicitly wrapped with AssertSqlSafe).
3. Temporarily restoring only organization FOR UPDATE after the first fix reproduced the FK cycle: `locking-fk-red.log`, exit **101**, Unavailable. Restored NO KEY UPDATE before GREEN. `locking-postgres-deadlocks.log` preserves both actual server deadlock records from this follow-up. This is a negative control, not an unresolved current defect.

## Barrier scenarios retained

`crates/infrastructure/tests/identity_locking.rs` runs five independently seeded scenarios sequentially in one bounded test, with runtime-role application calls and deployment-role fixture setup/observation:

- Real application change at its pre-retirement boundary versus proof issuance: change commits, stale proof request is explicitly rejected, membership/retirement/audit readback agrees. A concurrently queued login runs after change and its new credential authenticates.
- Two organizations with opposite shared actor/target pairs: both wait on the same lower identity; one commits and the other's retired caller is rejected; exactly one role-change audit commits. A third unrelated identity/organization completes a real change while both shared actors remain blocked, proving there is no global serialization gate.
- Login queued before role change: the login completes first, and the change retires that newly published session as well as the old session; zero live target sessions remain.
- Proof-first ordering: proof holds identity and waits on its session; role change holds organization serialization and waits on that identity. Release permits both to finish without the FK cycle.
- Actor expiry during an observed target-identity wait: wait through the actual database deadline, release, require Forbidden and unchanged membership, target session, proof consumption and audit count.

The earlier `proof_lock_order.rs` remains as the direct adapter regression, updated to obey the same upfront contract. It is **not** the only evidence of application integration. All barriers use observable database waits; expiry polls the database deadline, not an arbitrary sleep. Scenario waits have five-second bounds and the matrix has a thirty-second bound.

## Verification (actual exits)

All logs are under `docs/thermonuclear/`.

- `python3 scripts/dependencies.py up`: **0**, `locking-dependencies.log`. Sole ownership of the three disposable fixed-port containers. Installed Rust 1.99.1 verified before Cargo; no toolchain/host service installed.
- `TEST_DATABASE_URL=<disposable deployment DSN> cargo test --locked -p infrastructure --test identity_locking --test proof_lock_order --test auth --test sessions --test proof_binding --test proof_limits --test owner_race --test transaction --test memberships --test projects --test invitations --test rls --test rls_tables`: **0**, `locking-focused.log`, **16 passing test results**. The one new matrix result contains the five scenarios above. No test ignored.
- `cargo test --locked -p application -p domain`: **0**, `locking-core-tests.log`, **10 passing test results**, including compile-fail contract checks.
- `cargo clippy --locked -p application -p infrastructure --all-targets -- -D warnings`: **0**, `locking-clippy.log`.
- `SQLX_OFFLINE=true cargo check --locked --workspace --all-targets`: **0**, `locking-rust-check.log`.
- `DATABASE_URL=<disposable deployment DSN> SQLX_OFFLINE=false cargo check --locked -p infrastructure`: **0**, `locking-sqlx-online.log`.
- Owned Rust files formatted directly with rustfmt and `skip_children=true`; `git diff --check`: **0**.
- `python3 scripts/dependencies.py down`: **0**. `locking-cleanup.log` verifies each exact owned container is absent (`podman container exists` exit 1) and `.local/dependencies.json` is absent. Fixed ports released; no live application/worker was started by this follow-up.

## Harness corrections and residual limitations

- An expanded first attempt at the tests incorrectly required the second queued waiter to name the original blocker directly. PostgreSQL can report the earlier queued waiter instead. The barrier now accepts the observed direct/queued chain, excluding the first waiter; no timing delay or weakened success assertion was substituted.
- Running trigger DDL teardown concurrently with another scenario's transaction/fixture setup stalled the test harness (240-second command timeout, `locking-fixture-contention.log`). The final matrix is sequential inside its test binary; production operations within each scenario remain concurrent. Final normal Cargo execution requires no special test-thread flag and passes. Test-only DDL has generated UUID identifiers/tenant filters and is dropped on successful completion; a panic can leave such fixtures in the disposable database, which the dependency owner must discard. The owned database was removed.
- This fixes the demonstrated workflow cycles, not every possible deadlock involving arbitrary direct SQL, future operations or adapter consumers that violate the upfront contract. The contract is documented, not a new runtime lock-state machine. Callers must not add another identity after authority acquisition. New multi-organization mutation workflows would need their own ordered organization-lock design.
- Membership writes sharing an actor or target now serialize on those identities across tenants. This is deliberate, bounded to involved users, and necessary for global credential retirement/login ordering; no throughput claim or benchmark is made.
- Fresh live authority is checked after the identity/session/membership admission waits; the expiry regression covers the new upfront wait. This is not a claim of clock-based validity at the exact COMMIT instant after every possible later storage/audit wait. Existing proof-consumption predicates and transaction-held revocation locks remain; a universal commit-time expiry policy was not introduced.
- No cargo-sqlx CLI preparation was run (previous handoff found the CLI absent); checked query strings/metadata are untouched and both compilation modes passed. New dynamic statements are exercised against PostgreSQL, not covered by generated SQLx metadata.
- Full verifier, native/OCI journeys, migration compatibility, generated-artifact drift, fresh-checkout qualification and independent review remain the parent's gates on the combined tree. Earlier broader evidence is not relabeled as a new run. Implementation verification is not approval.

## Files owned by this follow-up

Modified: `crates/application/src/{memberships,projects}.rs`, `crates/infrastructure/src/{lib,memberships,projects}.rs`, `crates/infrastructure/tests/proof_lock_order.rs`, `docs/thermonuclear/{implementation-runtime,runtime-coordination}.md`.

Created: `crates/infrastructure/src/identity.rs`, `crates/infrastructure/tests/identity_locking.rs`, this report and the `locking-*.log` evidence. No scripts modified. Prior writers' changes in those shared files were preserved.
