"""Run actual launcher copies with fault executables, never the host daemon."""

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
import os, pathlib, signal, sys, time
root = pathlib.Path(os.environ['FAULT_ROOT'])
args = sys.argv[1:]
if pathlib.Path(sys.argv[0]).name == 'rustup':
    print(os.environ['RUST_VERSION'] + '-x86_64-unknown-linux-gnu')
elif args and args[0] == 'create':
    if '--cidfile' in args and os.environ['FAULT'] != 'unknown':
        pathlib.Path(args[args.index('--cidfile')+1]).write_text('a'*64)
    (root/'allocated').write_text(str(os.getpid()))
    if os.environ['FAULT'] in ('unknown', 'published'):
        time.sleep(60)
    sys.exit(7)
elif args and args[0] == 'rm':
    with (root/'removed').open('a') as f:
        f.write(args[-1]+'\\n')
    sys.exit(9 if args[-1] == 'b'*64 else 0)
"""


class TestDependencyLauncher(unittest.TestCase):
    def fixture(self, directory):
        root = Path(directory)
        shutil.copytree(ROOT / "scripts", root / "scripts")
        shutil.copy(ROOT / "rust-toolchain.toml", root)
        import tomllib

        version = tomllib.loads((root / "rust-toolchain.toml").read_text())[
            "toolchain"
        ]["channel"]
        binaries = root / "bin"
        binaries.mkdir()
        for name in ["podman", "rustup", "cargo", "bun", "openssl"]:
            executable = binaries / name
            executable.write_text(FAKE)
            executable.chmod(0o755)
        return root, dict(
            os.environ,
            PATH=f"{binaries}:{os.environ['PATH']}",
            FAULT_ROOT=str(root),
            RUST_VERSION=version,
        )

    def test_create_nonzero_recovers_exact_id_in_actual_launcher(self):
        with tempfile.TemporaryDirectory() as directory:
            root, env = self.fixture(directory)
            result = subprocess.run(
                [sys.executable, "scripts/dependencies.py", "up"],
                cwd=root,
                env=dict(env, FAULT="nonzero"),
                capture_output=True,
                text=True,
                timeout=10,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertTrue((root / "removed").exists(), result.stderr)
            self.assertEqual((root / "removed").read_text().splitlines(), ["a" * 64])
            self.assertFalse((root / ".local/dependencies.json").exists())

    def test_signals_during_create_retain_unknown_or_remove_exact_id(self):
        for launcher, mode in itertools.product(
            ["dependencies", "dev", "verify"], ["unknown", "published"]
        ):
            for sig in [signal.SIGINT, signal.SIGTERM]:
                with (
                    self.subTest(mode=mode, signal=sig),
                    tempfile.TemporaryDirectory() as directory,
                ):
                    root, env = self.fixture(directory)
                    process = subprocess.Popen(
                        [
                            sys.executable,
                            f"scripts/{launcher}.py",
                            *(["up"] if launcher == "dependencies" else []),
                        ],
                        cwd=root,
                        env=dict(env, FAULT=mode),
                        stdout=subprocess.PIPE,
                        stderr=subprocess.PIPE,
                        text=True,
                        start_new_session=True,
                    )
                    try:
                        deadline = time.monotonic() + 5
                        while (
                            not (root / "allocated").exists()
                            and process.poll() is None
                            and time.monotonic() < deadline
                        ):
                            time.sleep(0.01)
                        self.assertTrue((root / "allocated").exists())
                        process.send_signal(sig)
                        _, stderr = process.communicate(timeout=8)
                        self.assertNotEqual(process.returncode, 0)
                        if mode == "unknown":
                            self.assertIn(
                                "unknown",
                                (root / ".local/dependencies.json").read_text(),
                            )
                            self.assertIn("allocation unknown", stderr)
                            self.assertFalse((root / "removed").exists())
                        else:
                            self.assertTrue((root / "removed").exists(), stderr)
                            self.assertEqual(
                                (root / "removed").read_text().splitlines(), ["a" * 64]
                            )
                        if launcher == "verify":
                            results = {
                                r["gate"]: r
                                for r in json.loads(
                                    (root / "docs/evidence/results.json").read_text()
                                )
                            }
                            self.assertNotEqual(results["dependencies"]["exit_code"], 0)
                            self.assertEqual(
                                results["cleanup"]["exit_code"],
                                1 if mode == "unknown" else 0,
                            )
                        child = int((root / "allocated").read_text())
                        with self.assertRaises(ProcessLookupError):
                            os.kill(child, 0)
                    finally:
                        if process.poll() is None:
                            os.killpg(process.pid, signal.SIGKILL)
                            process.communicate()

    def test_down_attempts_all_ids_after_first_removal_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            root, env = self.fixture(directory)
            (root / ".local").mkdir()
            state = root / ".local/dependencies.json"
            state.write_text(json.dumps({"first": "b" * 64, "second": "c" * 64}))
            result = subprocess.run(
                [sys.executable, "scripts/dependencies.py", "down"],
                cwd=root,
                env=dict(env, FAULT="remove"),
                capture_output=True,
                text=True,
                timeout=10,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(
                (root / "removed").read_text().splitlines(), ["b" * 64, "c" * 64]
            )
            self.assertNotIn("second", json.loads(state.read_text()))


if __name__ == "__main__":
    unittest.main()
