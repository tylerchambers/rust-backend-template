# ITOOL-1 resolution — first cancellation during teardown

## Outcome

The reproduced ITOOL-1 defect is fixed. Full `python3 scripts/verify.py` exited **0** on the combined dirty checkout: **42/42 distinct gates passed**, including the new teardown regression and cleanup; **48 Rust tests passed**. Current run ID: `40d4c59d332643719a1510c240ebf6e3`, HEAD `36d86b89d5d3462197ca56c93b5bd9ddc77596b5`, 2026-11-09 12:38:31–12:40:12 UTC.

Authoritative current evidence is `docs/evidence/run.json`, matching `results.json`, and `itool-1-full-verification.log` (explicit `exit=0`). Programmatic reconciliation found no missing gates, failed/nonzero records, duplicate gate names, or mismatched run IDs. No `.local/*.json` ownership manifests or `.local/*.cid` files remain. `git diff --check` passed. Earlier implementation/review run IDs are historical, not this run.

## Narrow implementation

- `scripts/lifecycle.py`: added a nestable `defer_termination` context around container cleanup and the entire owned-process teardown loop. INT/TERM are remembered while existing bounded operations finish. Nested shields forward cancellation to the enclosing owner rather than aborting its next cleanup. Existing ignored signals remain ignored. Handlers are restored afterward. An already-unwinding primary failure is preserved and annotated; otherwise cancellation replays to the launcher's signal handler, preserving nonzero exit (130/143).
- `scripts/verify.py`: shields the complete cleanup phase spanning both the verifier container owner and the separate dependency owner. Cleanup failures still fail the cleanup evidence gate, but no longer replace an existing primary exception. Added `tooling_teardown` to the executed and expected gates.
- `tests/tooling_teardown.py`: three composed regression tests, each with four subcases (INT/TERM crossed with removal failure or primary failure). The actual dependency `down` entrypoint, real process owner, and actual verifier `main`/finally/evidence/container ownership execute in subprocesses. A marker proves the first removal or child termination has started before signaling. Assertions cover later IDs, retained failed ID, later-child termination and actual reaping, nonzero cancellation, and primary exception/overall evidence preservation.

The verifier fixture substitutes only unrelated preflight/dependency startup/workload in a temporary script copy. Its main/finally, both owners, signal machinery and Evidence remain real. Fake Podman is a subprocess-local fault boundary, not a claim of reproducing a Podman daemon defect. No production application, Rust, SDK, seed, dependency version, or unrelated resource policy changed in this continuation.

## RED → GREEN evidence

1. `itool-1-red.log`: initial test run reproduced skipped container owners; process cases also exposed a fixture pipe issue because stranded descendants inherited the launcher's captured streams. Preserved as initial diagnostic evidence.
2. `itool-1-red-composed.log`: before implementation, corrected the fixture to give worker children DEVNULL streams. All **12 subcases failed assertions**: later IDs/owners skipped, later children not terminated, or the primary exception replaced by cancellation. No fixture timeout/errors in this corrected RED run.
3. `itool-1-green.log`: same three regressions passed after implementation. Subsequent assertions additionally check child PID disappearance before fallback cleanup and the verifier's exact primary failure in its overall ledger.
4. `itool-1-focused.log`: final teardown, lifecycle, launcher, evidence and browser suites all passed, exit 0. Expected timeout/drift negative controls remain explicit in their output. Ruff formatting and lint checks passed for all three changed Python files.
5. `itool-1-full-verification.log`: fresh complete verification after all executable edits, exit 0. Canonical `docs/evidence/tooling_teardown.log` reruns the final regression. Full verification also exercised real PostgreSQL seed conflicts/cancellation, Rust tests, lint/security checks, release, generated drift, native/OCI browser and SDK flows, development shutdown and revision compatibility.

## Bounds and limits

The shield does not remove deadlines: container removal retains its 30-second command timeout and bounded subprocess termination; process ownership retains its configured graceful timeout and forced-reap timeout. Total teardown remains bounded by the registered resource count and existing per-operation deadlines. It does not add retries, name-based deletion, pruning, host installation, generic recovery infrastructure, or authority over unrelated resources. Failed container IDs remain persisted.

SIGKILL cannot execute Python cleanup. Cancellation regressions use real signals and real subprocesses with a fake container executable; normal real-container journeys passed in the full gate. This is local implementation and execution evidence, not an independent review, fresh-checkout qualification, hosted CI, publication, or deployment. No commit, push or publication was performed.
