"""Verify active dev API/worker shutdown for terminal-group INT and TERM."""

import os
import signal
import subprocess
import time
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
for termination in [signal.SIGINT, signal.SIGTERM]:
    with (ROOT / f"docs/evidence/dev-{termination.name}.log").open("w") as log:
        process = subprocess.Popen(
            ["python3", "scripts/dev.py"],
            cwd=ROOT,
            start_new_session=True,
            stdout=log,
            stderr=subprocess.STDOUT,
        )
    try:
        deadline = time.monotonic() + 120
        while time.monotonic() < deadline:
            try:
                with urllib.request.urlopen(
                    "http://localhost:58039/ready", timeout=1
                ) as response:
                    assert response.status == 200
                    break
            except OSError:
                assert process.poll() is None, "Dev process exited before readiness"
                time.sleep(0.05)
        else:
            raise RuntimeError("Dev readiness deadline exceeded")
        children = [
            int(pid)
            for pid in Path(f"/proc/{process.pid}/task/{process.pid}/children")
            .read_text()
            .split()
        ]
        assert len(children) == 2, (
            "Dev API and worker must both be active before interruption"
        )
        os.killpg(process.pid, termination)
        assert process.wait(timeout=30) != 0
        for child in children:
            try:
                os.kill(child, 0)
            except ProcessLookupError:
                pass
            else:
                raise AssertionError(f"Detached dev child {child} survived cleanup")
        try:
            os.killpg(process.pid, 0)
        except ProcessLookupError:
            pass
        else:
            raise AssertionError("Dev child process survived cleanup")
    finally:
        if process.poll() is None:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()
print("PASS: active API and worker are reaped on terminal-group SIGINT and SIGTERM")
