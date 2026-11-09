# Tooling remediation and combined-tree verification

## Outcome

TOOL-1 through TOOL-6 are implemented in the actual callers, not only helper stubs. The canonical `python3 scripts/verify.py` completed with exit **0**, **41/41 recorded gates passed**, and **48 Rust test results passed**. Run ID: `525a7f66b49c484bb9b79d7518cdad53`; source HEAD `36d86b89d5d3462197ca56c93b5bd9ddc77596b5`, dirty implementation tree, 2026-11-09 12:26:35–12:28:11 UTC. No commit, push, merge or deployment was performed.

Authority: `docs/evidence/run.json` (`completed`, explicit expected gates), matching `results.json`, and `docs/thermonuclear/combined-verification-final.log` (explicit `exit=0`). The final check also ran `git diff --check` successfully. `.local/*.json` and `.local/*.cid` inventories are empty. Podman contains exactly the pre-existing stopped containers inventoried before this work; no live containers remain, and no unrelated resource was removed.

## Finding disposition

- **TOOL-1:** Dependencies, verifier OCI/browser containers, and revision compatibility use `Containers`. Provisional records/cidfile paths/mount evidence are persisted before create; exact 64-hex IDs authorize removal. Failed create recovers cidfiles; ambiguous allocation stays recorded. Fixture images/ports are shared in `fixtures.py`. Known IDs are removed with their anonymous volumes; mounts are never deleted by this owner.
- **TOOL-2:** Shared subprocess sessions are bounded, terminated, escalated and reaped. Cleanup aggregates failures and attempts remaining resources. Dev invokes dependency startup in-process, and verifier invokes dependency/revision owners in-process rather than placing them under a process-group kill deadline. INT/TERM unwind the actual owning workflows. The native process owner preserves a primary exception while reporting all cleanup failures. Verifier cleanup is itself an observable failed gate if incomplete.
- **TOOL-3:** `Evidence` clears stale results at run start, records started gates before execution, streams command output to disk, records failures/timeouts/interruption in finally paths, and atomically publishes overall completion only after expected gates and cleanup pass. Run metadata contains source revision, dirty state, timestamps and run ID. Drift and lifecycle/assertion failures are recorded. Historical logs are explicitly distinguished in acceptance documentation; old filenames alone do not establish current-run provenance.
- **TOOL-4:** Fixture checks execute before COMMIT under bounded SQL timeouts and transaction-held table locks. Incompatible user, organization, membership and project shapes fail without publishing any ownership or partial inserts. Seed cancellation also terminates the uniquely attributed PostgreSQL backend: killing a Podman exec client alone was experimentally shown insufficient. No production schema or application seed coupling was added.
- **TOOL-5:** Existing partial Rust-only `check_rust()` integration was retained and verified across guarded Cargo, lint/fmt, architecture and lint-control entrypoints; service tools are not part of the Rust guard. Just migration/OpenAPI commands use guarded Cargo.
- **TOOL-6:** Unknown workspace members now fail architecture validation, preventing the unclassified adapter escape. Negative controls cover new/renamed members and existing forbidden edges. This remains a Cargo dependency policy, not a claim to detect arbitrary SQL or authorization logic in source text.
- **IHTTP-1:** Added explicit `rows.len() <= 50` and empty-terminal-page assertions to the public session traversal test. The final full Rust suite passes. No production pagination code changed; a no-LIMIT mutation was not run in this task.

## Regression evidence

- `tool-launcher-red.log` predates this continuation and reproduces original caller cleanup defects. Final `tooling_launchers.log`: 3 tests, including dependency/dev/verifier INT+TERM during unknown/published-ID allocation and retained uncertainty/failure ledger checks. These use subprocess-local fake executables, not a claimed real Podman allocation race.
- `tooling_lifecycle.log`: 3 tests covering absent/invalid/published IDs, nonzero creation, failed start, failed first removal with later removal still attempted, and first-child shutdown timeout with all children reaped and original failure retained. `tool-process-red.log` records the missing configurable deadline seam before that focused probe.
- `tooling_browser.log`: 1 test with timeout, INT and TERM subcases through the actual browser acceptance caller and shared owner. Unrelated HTTP/SDK gates and the container executable are explicit fault-model boundaries; real native/OCI browser journeys pass separately.
- `tooling_evidence.log`: 2 tests for stale-ledger replacement, partial stdout on timeout, drift failure and an outer failure after successful cleanup. RED logs: `tool-evidence-red.log`, `tool-overall-red.log` (initial missing interfaces, not independently reproduced historical runtime defects).
- `tooling_guards.log`: 2 subprocess tests for missing Rust before Cargo execution and unclassified architecture packages.
- `seed-conflicts.log`: 3 real PostgreSQL tests (multiple conflict/signal subcases), using independently named cloned databases, unrelated rows, exact row snapshots, reruns, partial user-ID collision, organization/project/membership conflicts, and INT+TERM after observing a blocked INSERT in `pg_stat_activity`. `tool-seed-red.log` shows four accepted conflicts before SQL validation; `tool-seed-signal-red.log` shows the backend survived both signals before cancellation cleanup. `tool-seed-green.log` preserves focused success; the combined gate reran all cases.
- `dev-lifecycle.log` plus `dev-SIGINT.log`/`dev-SIGTERM.log`: actual active API/worker shutdown, with exact detached child PID disappearance checked.
- Combined gate additionally passes pinned Ruff/Biome/Clippy/cargo-deny, lint negative controls, Bun audit, architecture, Rust tests, release, generated OpenAPI/SDK byte drift, TypeScript, native/OCI HTTP/SDK/Chromium/OpenMetrics/observability, OCI build, and real two-revision schema compatibility.

Earlier combined attempts are retained honestly: `combined-verification.log` failed on a Ruff import directive; `combined-verification-2.log` failed because the migrated compatibility readiness probe accidentally inherited check=True. Both failures were corrected and full gates rerun. `combined-verification-3.log` was successful before the final additional regressions. The final log is the authority for the current executable tree.

## Changed source paths in this tooling slice

- `scripts/architecture.py`, `scripts/cargo.py`, `scripts/dependencies.py`, `scripts/dev.py`, `scripts/evidence.py`, `scripts/fixtures.py`, `scripts/lifecycle.py`, `scripts/lint.py`, `scripts/preflight.py`, `scripts/seed.py`, `scripts/verify.py`
- `tests/dev_lifecycle.py`, `tests/lint_policy.py`, `tests/revision_compatibility.py`, `tests/seed_conflicts.py`, `tests/tooling_browser.py`, `tests/tooling_evidence.py`, `tests/tooling_guards.py`, `tests/tooling_launchers.py`, `tests/tooling_lifecycle.py`
- `dev/seed.sql`, `justfile`, `crates/api/tests/http.rs` (only the bounded-page assertions in this continuation)
- `docs/acceptance.md`, `docs/thermonuclear/disposition.md`, this report, retained RED/GREEN/combined logs, and canonical `docs/evidence` outputs named above. Earlier Rust/API/SDK implementation changes remain intact and were exercised together.

## Limits and handoff

Fresh independent tooling review remains parent-owned. This report is implementation evidence, not independent approval. No fresh clone, hosted CI, production deployment, actual pathological Podman allocation race, or external email delivery is claimed. SIGKILL cannot execute Python cleanup; durable ownership manifests remain the recovery evidence. Unknown allocation requires investigation, not name-based removal/pruning. The read/write tool repeatedly misclassified full reads as partial; edits used its explicitly recommended patch interface instead. No approval denial was bypassed, and no host service/toolchain was installed.
