# CORE implementation handoff

Source revision: `36d86b89d5d3462197ca56c93b5bd9ddc77596b5`, branch `refactor/thermonuclear-review`.

## Status and blocking coordination

- **CORE-2: implemented and locally verified.** Rejected rename preserves every field, including version exhaustion.
- **CORE-1: application/domain slice implemented and locally verified; production adapter integration pending.** Project consumers no longer receive membership or organization mutation capabilities. The existing project fake has no unsupported panic stubs and passes the unchanged shared behavioral contract.
- **Combined tree is not buildable yet:** `cargo check -p infrastructure` exits 101 because the runtime-owned `crates/infrastructure/src/projects.rs` still implements the previous traits. This is not a passing infrastructure result. Exact required moves are in [core-adapter-handoff.md](core-adapter-handoff.md). Core did not violate disjoint ownership by editing that file.
- No commit, push, self-approval, fixture lifecycle command, full suite, or broad formatter was run. Parent/runtime owns adapter integration, PostgreSQL regressions, combined gates and independent review.

## Authority and scope

Read CONTRIBUTING, README, ADR 001/002, acceptance ledger, review-core.json and disposition.md. Loaded thermonuclear review, engineering style and TDD skills. Exact installed Rust 1.99.1 was checked before Cargo; no toolchain installation.

Owned source changes only:

1. `crates/application/src/projects.rs`
   - Extract `TenantAuthority::current_role` from organization persistence.
   - Reduce `TenantTransaction` to shared audit/commit only.
   - Move project job publication into `ProjectRepository`.
   - Replace catch-all factory with explicit `ProjectUnitOfWork::begin_projects` and `OrganizationUnitOfWork::begin_organization`, returning narrow transaction trait objects.
   - Project and organization use cases retain their original policy, operation ordering, cache behavior and commit boundaries.
   - Add separate compile-fail doctests for `set_role` and `remove_member` through the project transaction boundary.
2. `crates/application/src/memberships.rs`
   - Add membership-owned transaction/factory contracts (`MembershipTransaction`, `MembershipUnitOfWork::begin_memberships`).
   - Replace only constructor/factory types and begin calls; preserve organization lock, authorization, proof consumption, last-owner checks, credential retirement, audit and single commit ordering.
3. `crates/application/tests/projects.rs`
   - Fake implements only tenant authority, project persistence/job publication and audit/commit.
   - Remove all 16 `unimplemented!` methods; no replacement false-success/error stubs.
   - Keep both existing behavioral tests and shared contract assertions unchanged.
   - File shrank from 240 to 177 lines after scoped rustfmt.
4. `crates/domain/src/lib.rs`
   - Compute checked next version before assigning name or version.
5. `crates/domain/tests/project.rs`
   - Add table-driven stale-version, invalid-name and exhausted-version rejection regression asserting identity, tenant, name, version and archived state all remain unchanged.
6. `crates/infrastructure/src/lib.rs`
   - Add one private concrete `tenant_work` constructor using the existing cancellation-safe `begin_tenant` and existing storage error mapper.
   - Implement all three consumer factories over the same existing concrete SQLx `TenantWork`; no extra pool, transaction, independent commit, generic repository framework or wrapper repository.
   - Runtime adapter must finish the method/impl moves before these compile.

## CORE-2 RED → GREEN

Command: `cargo +1.99.1 test --offline --locked -p domain --test project`.

- [core-2-red.log](core-2-red.log), exit **101**: existing test passed; new desired-behavior regression failed at the name assertion with `left: "After"`, `right: "Before"`. This reproduces the actual in-memory partial mutation; it does not assert persisted corruption.
- Minimal production fix: calculate `next_version` first, then assign both fields.
- [core-2-green.log](core-2-green.log), exit **0**: both project tests passed.

## CORE-1 RED → GREEN

The structural compile contract and negative capability checks were established before narrowing production ports.

1. Updated project-only fake to remove unrelated capabilities and use the intended project factory. `cargo +1.99.1 test --offline --locked -p application --test projects` exited **101**, [core-1-red.log](core-1-red.log). Compiler rejects missing new traits and explicitly requires the removed `MembershipRepository` / `OrganizationRepository` implementations. This is a structural compile RED, not a runtime defect regression.
2. Added compile-fail doctests to the original broad `TenantTransaction`, calling `set_role` and `remove_member`. `cargo +1.99.1 test --offline --locked -p application --doc` exited **101**, [core-1-boundary-red.log](core-1-boundary-red.log): both tests **compiled successfully despite being marked compile_fail**. This demonstrates the unwanted capabilities really existed.
3. Narrowed the production ports, moved the doctests to the actual `ProjectTransaction` returned to consumers, and changed no behavioral contract assertions.
4. `cargo +1.99.1 test --offline --locked -p application -p domain` exited **0**, [core-green.log](core-green.log): **10 passing results** (2 application behavioral tests, 5 domain integration tests, 2 project capability compile-fail doctests and 1 existing typed-ID compile-fail doctest). Empty harnesses also passed. Count parsed from actual test-result lines.

## Other actual verification

- Scoped `rustfmt +1.99.1 --edition 2024 --config skip_children=true` on precisely the six owned changed Rust files: exit 0. `skip_children` prevented formatting other writers' modules.
- `cargo +1.99.1 clippy --offline --locked -p application -p domain --all-targets --all-features -- -D warnings`: exit **0**, [core-clippy.log](core-clippy.log).
- `git diff --check`: exit **0** at inspection time.
- Inspected owned-file diffs: no shared behavioral assertion weakened; no migrations or SQL statements modified by core.
- `cargo +1.99.1 check --offline --locked -p infrastructure`: exit **101**, [core-infrastructure-check.log](core-infrastructure-check.log). Four diagnostics: old UnitOfWork import, current_role in the old organization impl, project_changed in the old transaction impl, project_changed missing from ProjectRepository. These are the exact outstanding runtime-owned moves, not unrelated test failures.

## Required integration work and remaining gates

Runtime/parent must apply [core-adapter-handoff.md](core-adapter-handoff.md) to `infrastructure/src/projects.rs`, retaining runtime locking/SQL changes. This file was outside core's strict write ownership; the conflict was reported immediately and saved rather than silently editing it. No change to bootstrap constructor calls should be needed: the existing `Arc<Database>` implements each narrow factory.

After adapter integration, on a stable combined snapshot:

- Re-run infrastructure/workspace compile and applicable Clippy.
- Under runtime-owned disposable PostgreSQL fixtures, run the existing project contract, organization/project service, transaction/River rollback, memberships, proof_binding and owner_race tests. In particular retain actual project/audit/job rollback and locked proof/last-owner authority behavior. No database integration success is claimed by core.
- Parent runs full ordinary gates and independent read-only verification. The earlier baseline does not verify these changes.

No migration or intended public HTTP behavior changed. API/session/tooling and preexisting docs/evidence modifications visible in git status belong to other workers/baseline; core did not edit or revert them.
