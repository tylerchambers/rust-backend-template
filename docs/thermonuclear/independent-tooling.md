# Independent final tooling review

## Verdict

**Changes requested: one reproduced remaining TOOL-2 blocker.** A first SIGINT/SIGTERM arriving during cleanup aborts the cleanup loop, leaving later known resources unattempted. The successful combined run is real, internally consistent evidence, but its signal tests do not cover this timing.

Reviewed HEAD: `36d86b89d5d3462197ca56c93b5bd9ddc77596b5`, with the final dirty implementation and new untracked tooling files. Review-only: no implementation edits, commits, publication, tool installation, real container startup, or full-suite rerun. This report is the only intentional repository write. Parent retains ownership of fresh full verification.

## ITOOL-1 — P2 / medium: first termination signal during teardown skips remaining resources

**Locations:** `scripts/lifecycle.py:69–84,99–112,199–225`; actual caller `scripts/dependencies.py:145–148`; verifier cleanup `scripts/verify.py:321–335`.

`termination_signals.stop()` disables subsequent signals and raises `SystemExit`. This protects cleanup only when the first signal arrived *before* cleanup began. When that first signal arrives while `Containers.cleanup()` is running `rm`, `run_command` stops its subprocess and rethrows `SystemExit`; the cleanup loop catches only `Exception`. Its per-record `finally` saves ownership, but iteration ends immediately. Later IDs are never attempted. The process-owner loop has the same exception boundary. In the verifier, an exception escaping `containers.cleanup()` also skips the separate dependency cleanup call.

**Independent reproduction:** copied the actual `scripts/` into a temporary scratch checkout, wrote a legacy manifest containing exact IDs `a` repeated 64 times and `b` repeated 64 times, and invoked the actual `scripts/dependencies.py down` with a fake Podman executable. That executable recorded each removal and blocked the first one. After an observable marker, sent the launcher SIGINT, then repeated with SIGTERM. No host daemon was used.

Observed results:

```json
{"signal":"SIGINT","exit":130,"attempts":["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"],"retained":["first","second"],"stderr":""}
{"signal":"SIGTERM","exit":143,"attempts":["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"],"retained":["first","second"],"stderr":""}
```

The exact IDs remain recoverable, so this is not loss of deletion authority or collateral removal. It is a demonstrated violation of the accepted all-resource cleanup guarantee. A user interrupting a slow `down`, or CI terminating a normally finishing verification, can strand every later resource despite an otherwise healthy runtime.

**Remedy:** make teardown a bounded cancellation-shielded phase, or defer/rethrow the first cancellation only after every registered owner has been attempted. Preserve the original interruption/primary failure and failed IDs. Do not simply swallow cancellation and report success. Apply the policy to both process and container cleanup and the verifier's independent owners.

**Required regression:** send INT and TERM after an observed first removal/termination begins during otherwise normal teardown; assert later exact IDs/children are attempted/reaped, failures remain recorded, and exit remains nonzero. Include the verifier's separate dependency owner, not only an isolated helper. Existing tests cover signals during creation/browser execution and ordinary removal failures, not the first signal during cleanup.

### Durable reproduction source

The following standalone probe uses only temporary copies and a fake executable. Run from this checkout with Python; set `TMPDIR` to the designated scratch directory. It preserves the exact exercised composition and fault boundary.

```python
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time

source = Path.cwd()
fake = '''#!/usr/bin/env python3
import os,pathlib,sys,time
root=pathlib.Path(os.environ['PROBE_ROOT'])
with (root/'attempts').open('a') as f: f.write(sys.argv[-1]+'\\n')
if sys.argv[-1]=='a'*64:
 (root/'active').touch()
 time.sleep(60)
'''
for sig in [signal.SIGINT, signal.SIGTERM]:
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        shutil.copytree(source / 'scripts', root / 'scripts')
        (root / '.local').mkdir()
        (root / 'bin').mkdir()
        state = root / '.local/dependencies.json'
        state.write_text(json.dumps({'first': 'a' * 64, 'second': 'b' * 64}))
        executable = root / 'bin/podman'
        executable.write_text(fake)
        executable.chmod(0o755)
        child = subprocess.Popen(
            [sys.executable, 'scripts/dependencies.py', 'down'], cwd=root,
            env=dict(os.environ, PATH=str(root / 'bin') + ':' + os.environ['PATH'],
                     PROBE_ROOT=directory),
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
        )
        try:
            deadline = time.monotonic() + 5
            while not (root / 'active').exists() and time.monotonic() < deadline:
                time.sleep(0.01)
            assert (root / 'active').exists()
            child.send_signal(sig)
            _, errors = child.communicate(timeout=8)
            print(sig.name, child.returncode,
                  (root / 'attempts').read_text().splitlines(),
                  list(json.loads(state.read_text())), repr(errors))
        finally:
            if child.poll() is None:
                child.kill()
                child.communicate()
```

## Accepted-scope assessment

| Finding / requirement | Independent assessment |
|---|---|
| TOOL-1: exact-ID lifecycle | Substantially resolved. Dependencies, compatibility, OCI server/worker and browser callers use the shared owner. Provisional manifest/cidfile/mount obligations precede create. Missing or invalid cidfiles retain unknown allocation; stdout/names do not authorize removal. Published IDs are recovered after failed creation. Removal validates a complete lowercase 64-hex ID and includes anonymous volumes. Failed removals retain authority; mounted backing paths are not deleted. |
| TOOL-2: all-resource and signal cleanup | Ordinary exceptions/timeouts are aggregated and later resources are attempted. Process sessions are terminated, escalated and reaped. Creation and browser cancellation are exercised through actual callers. **Not closed: ITOOL-1 above.** |
| TOOL-3: truthful evidence | Resolved within the accepted workflow. Initialization replaces prior results and marks a new run running before work. Gates start before commands; output streams to files; failures/timeouts/interruptions update results. The overall session rejects missing/nonpassing gates and marks failures outside individual gates. Cleanup and generated drift are recorded. Stale files are explicitly historical in acceptance documentation. No success is inferred solely from old log filenames. |
| TOOL-4: atomic seed validation | SQL validates fixed user, organizations, memberships and projects before COMMIT in the same transaction. Table locks serialize fixture publication; lock/statement timeouts bound SQL waits. ON_ERROR_STOP plus connection failure rolls back the transaction. Compatible reruns do not rewrite ownership; incompatible shapes fail atomically. |
| Seed backend termination safety | The launcher selects the checkout-owned exact Postgres container and sets a fresh UUID-derived `PGAPPNAME`. Cancellation cleanup filters both `current_database()` and that exact generated application name, rather than terminating all application sessions or guessing a PID. The name is locally generated hexadecimal, not SQL supplied by a caller. Cleanup is bounded and a failure is attached to the original error. This is appropriately narrow disposable-fixture authority, not a claim of protection against a malicious database superuser. |
| TOOL-5: Rust preflight | Shared Rust-only guard precedes lint/fmt, architecture, lint controls, compatibility and guarded Cargo recipes. Missing-toolchain controls fail before the Cargo sentinel. The guard does not add service prerequisites to standalone Cargo. |
| TOOL-6: closed architecture | Unknown metadata packages are rejected rather than silently skipped. Existing edge restrictions remain intact. This is Cargo dependency classification, not arbitrary transitive/source-policy analysis. |
| API page-size assertion | `crates/api/tests/http.rs` now asserts each page has at most 50 entries, traverses to an empty terminal page, and compares all live session IDs in order. The data set exceeds one page and excludes foreign/expired/revoked/idle sessions. This closes the identified assertion gap; no mutation-test execution is claimed. |

The shared `Containers`/process ownership and `Evidence` separation removes duplicated lifecycle orchestration rather than introducing a generic recovery framework. Largest tooling source is `scripts/verify.py` at 339 lines; no 1,000-line crossing or additional structural blocker was found. Existing repository unittest/standard-library conventions were retained; this bounded remediation is not a request to replace its tooling framework.

## Independent verification and evidence reconciliation

Parsed `docs/evidence/run.json` and `results.json` programmatically:

- Run ID exactly `525a7f66b49c484bb9b79d7518cdad53`.
- Overall `completed`, dirty HEAD correctly recorded, start `2026-11-09T12:26:35.051268+00:00`, finish `2026-11-09T12:28:11.044318+00:00`.
- 41 records, 41 distinct gate names, no missing expected gates, no nonpassing/incorrect-run-ID/nonzero records; cleanup passed.
- Independently summed Cargo's actual `test result: ok` lines in `rust-tests.log`: 48 passed tests.
- Read combined final log: all recorded gate outputs present and explicit `exit=0`. Read seed-conflict, lifecycle, launcher, guard, evidence, browser and compatibility logs; these agree with the implementation report. Native/OCI browser command logs contain actual success output, and compatibility records its prior revision.
- Current `.local/*.json` and `.local/*.cid` inventories were empty. This review does not independently certify the report's historical inventory of unrelated stopped containers.
- `git diff --check`: exit 0.

Independently executed the existing nonresource test scripts, without rewriting canonical evidence:

```text
python3 tests/tooling_lifecycle.py   — 3 tests OK
python3 tests/tooling_launchers.py   — 3 tests OK
python3 tests/tooling_guards.py      — 2 tests OK
python3 tests/tooling_evidence.py    — 2 tests OK
combined command exit: 0
```

The lifecycle suite deliberately reports graceful-shutdown timeouts while proving forced reaping and preservation of the original failure. The evidence suite deliberately records timeout 124 and drift 1 while its assertions pass. These are expected negative controls, not hidden failed gates.

Real PostgreSQL seed tests were source-reviewed and their existing combined execution output checked, not rerun by this reviewer. Browser fault tests bind a fixed port, so they were inspected but not rerun concurrently with the parent. No real pathological Podman allocation race, fresh checkout, hosted CI, full verifier rerun, or deployment is claimed. The ledger identifies a dirty tree, not a cryptographic snapshot of every dirty file; parent-owned fresh combined verification remains necessary after any repair.

## Reviewed files

Complete final contents and relevant tracked deltas/new files reviewed:

- `scripts/architecture.py`, `scripts/cargo.py`, `scripts/dependencies.py`, `scripts/dev.py`, `scripts/evidence.py`, `scripts/fixtures.py`, `scripts/lifecycle.py`, `scripts/lint.py`, `scripts/preflight.py`, `scripts/seed.py`, `scripts/verify.py`.
- `tests/dev_lifecycle.py`, `tests/lint_policy.py`, `tests/revision_compatibility.py`, `tests/seed_conflicts.py`, `tests/tooling_browser.py`, `tests/tooling_evidence.py`, `tests/tooling_guards.py`, `tests/tooling_launchers.py`, `tests/tooling_lifecycle.py`.
- `dev/seed.sql`, `justfile`, `pyproject.toml`, `docs/acceptance.md`; relevant final diff and session-pagination assertions in `crates/api/tests/http.rs` (not a second whole-file HTTP approval).
- Governing `docs/thermonuclear/review-tooling.json`, `implementation-tooling.md`, `disposition.md`; evidence ledger and the concrete logs named above. Unrelated Rust/API/SDK implementation remains assigned to its separate reviewers.

## Handoff

Repair ITOOL-1 with a regression for first cancellation during teardown, then rerun focused lifecycle/caller tests and parent-owned final gates. TOOL-1/3/4/5/6 have no additional demonstrated blocker in this accepted scope. Do not describe TOOL-2 as fully closed solely because the current 41-gate run passes.
