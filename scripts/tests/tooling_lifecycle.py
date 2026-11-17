"""Fault-model tests at real subprocess boundaries; no container daemon."""

import json
import os
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

FAKE = """#!/usr/bin/env python3
import os, pathlib, sys, time
args = sys.argv[1:]
root = pathlib.Path(os.environ["FAULT_ROOT"])
mode = os.environ.get("FAULT", "ok")
if args[0] == "create":
    cidfile = pathlib.Path(args[args.index("--cidfile") + 1])
    (root / "allocated").write_text("allocated")
    if mode != "unknown":
        cidfile.write_text("bad" if mode == "invalid" else "a" * 64)
    if mode in ("unknown", "published", "invalid"):
        time.sleep(30)
    print("malformed stdout" if mode == "nonzero" else "a" * 64)
    sys.exit(3 if mode == "nonzero" else 0)
elif args[0] == "rm":
    with (root / "removed").open("a") as output:
        output.write(args[-1] + "\\n")
    sys.exit(4 if args[-1] == "b" * 64 else 0)
elif args[0] == "start":
    sys.exit(5 if mode == "start" else 0)
"""


class TestProcessOwnership(unittest.TestCase):
    def test_first_timeout_still_reaps_every_child_and_preserves_primary(self):
        from lifecycle import processes

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            children = []
            primary = ValueError("primary gate failure")
            try:
                with self.assertRaises(ValueError) as caught:
                    with processes(timeout=0.1) as children:
                        for number in range(2):
                            marker = root / str(number)
                            child = subprocess.Popen(
                                [
                                    sys.executable,
                                    "-c",
                                    "import pathlib,signal,time; signal.signal(signal.SIGTERM, signal.SIG_IGN); "
                                    f"pathlib.Path({str(marker)!r}).touch(); time.sleep(60)",
                                ],
                                start_new_session=True,
                            )
                            children.append(child)
                            deadline = time.monotonic() + 5
                            while not marker.exists() and time.monotonic() < deadline:
                                time.sleep(0.01)
                            self.assertTrue(marker.exists())
                        raise primary
                self.assertIs(caught.exception, primary)
                self.assertTrue(all(child.poll() is not None for child in children))
                self.assertIn("graceful shutdown timed out", str(primary.__notes__))
            finally:
                for child in children:
                    if child.poll() is None:
                        child.kill()
                        child.wait()


class TestContainerOwnership(unittest.TestCase):
    def test_timeout_retains_unknown_but_removes_recovered_id(self):
        from lifecycle import Containers

        for mode in ["unknown", "invalid", "published", "nonzero", "start"]:
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as directory:
                temp = Path(directory)
                executable = temp / "podman"
                executable.write_text(FAKE)
                executable.chmod(0o755)
                environment = dict(os.environ, FAULT_ROOT=str(temp), FAULT=mode)
                owner = Containers(
                    temp / "state.json",
                    executable=str(executable),
                    environment=environment,
                )
                backing = temp / "backing"
                backing.mkdir()
                (backing / "sentinel").write_text("retain me")
                with self.assertRaises((subprocess.SubprocessError, RuntimeError)):
                    container = owner.create(
                        "fixture", ["image"], mounts=[backing], timeout=0.2
                    )
                    owner.start(container)
                errors = owner.cleanup()
                if mode in ["unknown", "invalid"]:
                    self.assertTrue(errors)
                    self.assertTrue((temp / "state.json").exists())
                    self.assertIn("unknown", (temp / "state.json").read_text())
                    self.assertFalse((temp / "removed").exists())
                else:
                    self.assertEqual(errors, [])
                    self.assertEqual(
                        (temp / "removed").read_text().splitlines(), ["a" * 64]
                    )
                    self.assertFalse((temp / "state.json").exists())
                self.assertEqual((backing / "sentinel").read_text(), "retain me")

    def test_failed_removal_keeps_id_and_attempts_remaining(self):
        from lifecycle import Containers

        with tempfile.TemporaryDirectory() as directory:
            temp = Path(directory)
            executable = temp / "podman"
            executable.write_text(FAKE)
            executable.chmod(0o755)
            state = temp / "state.json"
            state.write_text(json.dumps({"first": "b" * 64, "second": "c" * 64}))
            owner = Containers(
                state,
                executable=str(executable),
                environment=dict(os.environ, FAULT_ROOT=str(temp)),
            )
            self.assertTrue(owner.cleanup())
            self.assertEqual(
                set((temp / "removed").read_text().splitlines()), {"b" * 64, "c" * 64}
            )
            self.assertIn("b" * 64, state.read_text())
            self.assertNotIn("c" * 64, state.read_text())


if __name__ == "__main__":
    unittest.main()
