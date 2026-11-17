"""First-signal teardown regressions through real owners and launcher composition."""

import itertools
import json
import os
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FAKE = """#!/usr/bin/env python3
import os,pathlib,sys,time
root=pathlib.Path(os.environ['FAULT_ROOT'])
with (root/'attempts').open('a') as f:
    f.write(sys.argv[-1]+'\\n')
if sys.argv[-1]=='a'*64:
    (root/'active').write_text(str(os.getpid()))
    deadline=time.monotonic()+5
    while not (root/'release').exists() and time.monotonic()<deadline:
        time.sleep(.01)
    sys.exit(9 if os.environ['FAIL_REMOVE']=='1' else 0)
"""
PROCESS = """
import os,pathlib,signal,subprocess,sys,time
sys.path.insert(0,'scripts')
from lifecycle import processes,termination_signals
root=pathlib.Path.cwd()
worker='''import os,pathlib,signal,time,sys
root=pathlib.Path.cwd()
number=sys.argv[1]
def stop(sig,frame):
    (root/('stopping-'+number)).touch()
    if number=='0':
        (root/'active').write_text(str(os.getpid()))
    else:
        sys.exit(0)
signal.signal(signal.SIGTERM,stop)
(root/('ready-'+number)).write_text(str(os.getpid()))
while True: time.sleep(.01)
'''
try:
    with termination_signals(), processes(timeout=.4) as children:
        for number in range(2):
            children.append(subprocess.Popen([sys.executable,'-c',worker,str(number)],start_new_session=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL))
            deadline=time.monotonic()+5
            while not (root/('ready-'+str(number))).exists():
                assert time.monotonic()<deadline
                time.sleep(.01)
        if os.environ['PRIMARY']=='1':
            raise ValueError('primary gate failure')
finally:
    (root/'reaped').write_text(str([p.returncode for p in children]))
"""


class TestTeardownCancellation(unittest.TestCase):
    def fixture(self, directory):
        root = Path(directory)
        shutil.copytree(ROOT / "scripts", root / "scripts")
        (root / "bin").mkdir()
        (root / ".local").mkdir()
        executable = root / "bin/podman"
        executable.write_text(FAKE)
        executable.chmod(0o755)
        return root

    def interrupt(self, root, command, sig, *, primary=False, failure=False):
        process = subprocess.Popen(
            command,
            cwd=root,
            env=dict(
                os.environ,
                PATH=f"{root / 'bin'}:{os.environ['PATH']}",
                FAULT_ROOT=str(root),
                PRIMARY=str(int(primary)),
                FAIL_REMOVE=str(int(failure)),
            ),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            start_new_session=True,
        )
        try:
            deadline = time.monotonic() + 8
            while not (root / "active").exists() and time.monotonic() < deadline:
                if process.poll() is not None:
                    self.fail(process.communicate()[1])
                time.sleep(0.01)
            self.assertTrue((root / "active").exists())
            process.send_signal(sig)
            # Keep removal in flight briefly after sending the signal.
            # The process teardown case instead exercises bounded escalation.
            time.sleep(0.05)
            (root / "release").touch()
            stdout, stderr = process.communicate(timeout=10)
            self.assertNotEqual(process.returncode, 0, stdout + stderr)
            if primary:
                self.assertIn("ValueError: primary gate failure", stderr)
            elif not failure:
                self.assertEqual(process.returncode, 128 + sig, stderr)
            for marker in root.glob("ready-*"):
                with self.assertRaises(ProcessLookupError):
                    os.kill(int(marker.read_text()), 0)
            return stderr
        finally:
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGKILL)
                process.communicate()
            # RED can strand owned workers. Never leave test resources behind.
            for marker in root.glob("ready-*"):
                try:
                    os.killpg(int(marker.read_text()), signal.SIGKILL)
                except ProcessLookupError:
                    pass

    def test_down_finishes_later_ids_and_retains_failed_removal(self):
        for sig, failure in itertools.product(
            [signal.SIGINT, signal.SIGTERM], [False, True]
        ):
            with (
                self.subTest(signal=sig, failure=failure),
                tempfile.TemporaryDirectory() as directory,
            ):
                root = self.fixture(directory)
                state = root / ".local/dependencies.json"
                state.write_text(json.dumps({"first": "a" * 64, "later": "b" * 64}))
                self.interrupt(
                    root,
                    [sys.executable, "scripts/dependencies.py", "down"],
                    sig,
                    failure=failure,
                )
                self.assertEqual(
                    (root / "attempts").read_text().splitlines(), ["a" * 64, "b" * 64]
                )
                if failure:
                    self.assertEqual(set(json.loads(state.read_text())), {"first"})
                    self.assertIn("a" * 64, state.read_text())
                else:
                    self.assertFalse(state.exists())

    def test_processes_reap_later_children_and_preserve_primary(self):
        for sig, primary in itertools.product(
            [signal.SIGINT, signal.SIGTERM], [False, True]
        ):
            with (
                self.subTest(signal=sig, primary=primary),
                tempfile.TemporaryDirectory() as directory,
            ):
                root = self.fixture(directory)
                self.interrupt(
                    root, [sys.executable, "-c", PROCESS], sig, primary=primary
                )
                self.assertTrue((root / "stopping-1").exists())
                self.assertNotIn("None", (root / "reaped").read_text())
                for marker in root.glob("ready-*"):
                    with self.assertRaises(ProcessLookupError):
                        os.kill(int(marker.read_text()), 0)

    def test_verifier_attempts_separate_dependency_owner(self):
        for sig, primary in itertools.product(
            [signal.SIGINT, signal.SIGTERM], [False, True]
        ):
            with (
                self.subTest(signal=sig, primary=primary),
                tempfile.TemporaryDirectory() as directory,
            ):
                root = self.fixture(directory)
                # Only unrelated setup/workload is substituted in the copied script.
                # main(), its finally, Evidence and both Containers owners stay real.
                (root / "scripts/preflight.py").write_text("def check(): pass\n")
                (root / "scripts/dependencies.py").write_text(
                    "import json,pathlib\ndef up():\n"
                    "    pathlib.Path('.local/dependencies.json').write_text(json.dumps({'dependency':'c'*64}))\n"
                )
                path = root / "scripts/verify.py"
                source = path.read_text()
                start = source.index("def verify(evidence, containers):")
                end = source.index("\ndef main():", start)
                path.write_text(
                    source[:start]
                    + """def verify(evidence, containers):
    containers.resources = {'first': {'id':'a'*64,'cidfile':None}, 'later': {'id':'b'*64,'cidfile':None}}
    containers.save()
    if os.environ['PRIMARY']=='1':
        raise ValueError('primary gate failure')

"""
                    + source[end:]
                )
                self.interrupt(
                    root,
                    [sys.executable, "scripts/verify.py"],
                    sig,
                    primary=primary,
                    failure=True,
                )
                self.assertEqual(
                    (root / "attempts").read_text().splitlines(),
                    ["a" * 64, "b" * 64, "c" * 64],
                )
                self.assertFalse((root / ".local/dependencies.json").exists())
                states = list((root / ".local").glob("verify-*.json"))
                self.assertEqual(len(states), 1)
                self.assertEqual(set(json.loads(states[0].read_text())), {"first"})
                evidence = root / "docs/evidence"
                metadata = json.loads((evidence / "run.json").read_text())
                self.assertEqual(metadata["status"], "failed")
                if primary:
                    self.assertEqual(metadata["error"], "primary gate failure")
                cleanup = next(
                    r
                    for r in json.loads((evidence / "results.json").read_text())
                    if r["gate"] == "cleanup"
                )
                self.assertEqual(cleanup["status"], "failed")


if __name__ == "__main__":
    unittest.main()
