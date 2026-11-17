"""Browser gate fault model: exercise the real acceptance caller and owner."""

import http.server
import os
import shutil
import signal
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FAKE = """#!/usr/bin/env python3
import os,pathlib,sys,time
root=pathlib.Path(os.environ['FAULT_ROOT'])
a=sys.argv[1:]
if pathlib.Path(sys.argv[0]).name=='bun':
    sys.exit(0)
if a[0]=='create':
    pathlib.Path(a[a.index('--cidfile')+1]).write_text('a'*64)
elif a[0]=='start':
    (root/'active').write_text(str(os.getpid()))
    time.sleep(60)
elif a[0]=='rm':
    (root/'removed').write_text(a[-1])
"""
CHILD = """
import pathlib,sys
sys.path.insert(0,'scripts')
from verify import acceptance
from evidence import Evidence
from lifecycle import Containers,termination_signals
root=pathlib.Path.cwd()
evidence=Evidence(root/'docs/evidence')
with termination_signals(), Containers(root/'state.json') as owner:
    acceptance('native',evidence,owner,browser_timeout=float(sys.argv[1]))
"""


class Ready(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.end_headers()

    def log_message(self, *args):
        pass


class TestBrowserGate(unittest.TestCase):
    def test_timeout_and_signals_cleanup_the_created_container(self):
        server = http.server.ThreadingHTTPServer(("127.0.0.1", 58039), Ready)
        thread = threading.Thread(target=server.serve_forever)
        thread.start()
        try:
            for sig in [None, signal.SIGINT, signal.SIGTERM]:
                with (
                    self.subTest(signal=sig),
                    tempfile.TemporaryDirectory() as directory,
                ):
                    root = Path(directory)
                    shutil.copytree(ROOT / "scripts", root / "scripts")
                    for path in ["tests/system", "sdk", ".venv/bin", "bin"]:
                        (root / path).mkdir(parents=True)
                    for name in ["http_acceptance", "metrics"]:
                        (root / f"tests/system/{name}.py").write_text(
                            "# Unrelated gate fault-model boundary.\n"
                        )
                    (root / ".venv/bin/python").symlink_to(sys.executable)
                    for name in ["podman", "bun"]:
                        executable = root / "bin" / name
                        executable.write_text(FAKE)
                        executable.chmod(0o755)
                    process = subprocess.Popen(
                        [sys.executable, "-c", CHILD, "0.2" if sig is None else "60"],
                        cwd=root,
                        env=dict(
                            os.environ,
                            PATH=f"{root / 'bin'}:{os.environ['PATH']}",
                            FAULT_ROOT=str(root),
                        ),
                        stdout=subprocess.PIPE,
                        stderr=subprocess.PIPE,
                        text=True,
                        start_new_session=True,
                    )
                    try:
                        deadline = time.monotonic() + 5
                        while (
                            not (root / "active").exists()
                            and process.poll() is None
                            and time.monotonic() < deadline
                        ):
                            time.sleep(0.01)
                        self.assertTrue(
                            (root / "active").exists(),
                            process.communicate()[1]
                            if process.poll() is not None
                            else "no active browser",
                        )
                        if sig is not None:
                            process.send_signal(sig)
                        _, errors = process.communicate(timeout=8)
                        self.assertNotEqual(process.returncode, 0)
                        self.assertEqual(
                            (root / "removed").read_text(), "a" * 64, errors
                        )
                        self.assertFalse((root / "state.json").exists())
                        with self.assertRaises(ProcessLookupError):
                            os.kill(int((root / "active").read_text()), 0)
                    finally:
                        if process.poll() is None:
                            process.kill()
                            process.communicate()
        finally:
            server.shutdown()
            thread.join()
            server.server_close()


if __name__ == "__main__":
    unittest.main()
