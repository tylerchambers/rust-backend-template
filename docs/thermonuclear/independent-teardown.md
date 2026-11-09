# Independent targeted teardown review

## Verdict

**ITOOL-1 closed in the reviewed working tree. No remaining blocker found within this targeted fix.** The first SIGINT/SIGTERM during teardown no longer skips later registered resources or the verifier's separate dependency owner. Cancellation does not become success, the original workload failure remains primary, and failed container IDs remain recorded.

Reviewed HEAD: `36d86b89d5d3462197ca56c93b5bd9ddc77596b5`, plus existing dirty/untracked implementation. This is not approval of a clean committed snapshot or publication. Loaded the thermonuclear review and Tyler engineering skills. Compared the actual implementation against `independent-tooling.md` and `itool-1-resolution.md`, rather than treating the implementation report as independent proof.

## Scope and findings

Full files reviewed: `scripts/lifecycle.py`, `scripts/verify.py`, `tests/tooling_teardown.py`, and the two reports above. Supporting inspection: `scripts/evidence.py` and the dependency launcher cleanup/entrypoint in `scripts/dependencies.py:137–156`. The requested `verify.py` is located under `scripts/`, not at repository root.

- **Bounded signal deferral:** `lifecycle.py:87–121` retains the first INT/TERM, restores handlers, respects already-ignored signals and forwards cancellation to an enclosing shield. Container cleanup (`237–264`) and the complete process-owner loop (`141–150`) use that policy. Container commands retain a 30-second timeout; subprocess termination retains graceful and forced-reap deadlines (`21–41`). The process regression exercises graceful-timeout escalation. Bounds are per registered resource and assume normally functioning OS process/filesystem operations; this is not a hard real-time guarantee or SIGKILL recovery.
- **All owners attempted:** the container loop aggregates ordinary failures and persists each iteration; the process loop continues after termination failures. The verifier shields both container owners together (`verify.py:324–340`), so replay from an inner shield cannot bypass dependency cleanup. The composed regression observes removal attempts for `a`, `b`, then `c` exact IDs despite the first removal failing and a signal arriving during it.
- **Original failure semantics:** `defer_termination` captures the already-unwinding exception and adds a signal note instead of replacing it. The verifier separately records a failed cleanup gate while attaching cleanup diagnostics to the original failure. Process and verifier regressions preserve `ValueError: primary gate failure`; verifier metadata retains that exact primary error.
- **No successful cancellation:** absent an existing primary failure, the saved signal is replayed to the launcher handler, producing a nonzero exit. Successful-removal dependency cases assert exact 130/143 exits. Every regression asserts nonzero exit; verifier cases assert failed overall evidence and failed cleanup evidence. A prior workload failure remains a failure rather than being converted to success or replaced by cancellation.
- **Retained IDs:** failed removal retains the first exact ID in its ownership manifest; successful later removals disappear. The dependency and verifier regressions assert this state. No names, pruning, or new deletion authority were introduced.
- **Structure:** one focused lifecycle context is reused at owner boundaries; the verifier adds the necessary outer ownership boundary. No new structural blocker or unrelated scope expansion found.

The regressions use observable first-operation markers before signaling, real signals and subprocesses, and a fake Podman executable. The verifier fixture substitutes unrelated startup/workload only; its actual `main`, cleanup composition, Evidence implementation, and container owners execute. Process assertions check reaping and PID disappearance before fallback fixture cleanup. These are meaningful regressions for the reported timing, not a claim of exercising a real container daemon under fault.

## Actual independent execution

From `/home/hermes/rust-backend-template-thermonuclear`:

```text
PYTHONDONTWRITEBYTECODE=1 TMPDIR=/home/hermes/.hermes/cache/scratch python3 tests/tooling_teardown.py -v

test_down_finishes_later_ids_and_retains_failed_removal ... ok
test_processes_reap_later_children_and_preserve_primary ... ok
test_verifier_attempts_separate_dependency_owner ... ok

Ran 3 tests in 3.212s
OK
exit_code: 0
```

The three test methods cover 12 subcases (each crosses INT/TERM with removal failure or primary failure). `git diff --check` also exited 0.

Reviewed executable SHA-256 values:

```text
6034cba0f68762808792df9d00cea3797a9bf37edb39f4c0e901cfab62e53943  scripts/lifecycle.py
fce28e77490dd85e2471d4333de5238a1e533110bc191a00d56b603e46ee40e6  scripts/verify.py
4cb20eab88513c03913d56c20156d32e7ed18c732ab30c48469d6b4289f6bd97  tests/tooling_teardown.py
```

## Limits and handoff

No full gate, actual container invocation, implementation edit, commit, push, or publication was performed. The existing report's 42-gate/48-Rust-test run was not rerun or independently reconciled in this narrow review. Fresh-checkout qualification remains parent-owned. No execution blocker encountered. The only intentional repository write by this reviewer is this report; existing implementation and canonical evidence were left unchanged.
