"""Isolated executable controls: never invoke real Cargo or install tools."""

import json
import os
import subprocess
import sys
import tempfile
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


class TestToolchainGuard(unittest.TestCase):
    def test_absent_rust_stops_every_entrypoint_before_cargo(self):
        with tempfile.TemporaryDirectory() as directory:
            temp = Path(directory)
            for name, body in {
                "rustup": "exit 0",
                "cargo": ': > "$SENTINEL"; exit 99',
                "bun": "printf '1.4.4\\n'",
                "uv": "exit 0",
            }.items():
                executable = temp / name
                executable.write_text("#!/bin/sh\n" + body + "\n")
                executable.chmod(0o755)
            environment = dict(
                os.environ, PATH=str(temp), SENTINEL=str(temp / "cargo-called")
            )
            for arguments in [
                ["scripts/lint.py", "fmt"],
                ["scripts/lint.py", "check"],
                ["scripts/architecture.py"],
                ["scripts/tests/lint_policy.py"],
                ["scripts/cargo.py", "run", "--locked", "-p", "cli", "--", "migrate"],
                ["scripts/cargo.py", "run", "--locked", "-p", "cli", "--", "openapi"],
            ]:
                with self.subTest(arguments=arguments):
                    result = subprocess.run(
                        [sys.executable, *arguments],
                        cwd=ROOT,
                        env=environment,
                        capture_output=True,
                        text=True,
                        timeout=10,
                    )
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn(
                        "refusing implicit rustup installation", result.stderr
                    )
                    self.assertFalse((temp / "cargo-called").exists())


class TestArchitecture(unittest.TestCase):
    def test_unknown_workspace_members_fail_closed(self):
        version = tomllib.loads((ROOT / "rust-toolchain.toml").read_text())[
            "toolchain"
        ]["channel"]
        for name in ["new-adapter", "renamed-domain"]:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                temp = Path(directory)
                packages = [
                    {"name": "domain", "dependencies": [{"name": name, "kind": None}]},
                    {"name": name, "dependencies": [{"name": "sqlx", "kind": None}]},
                ]
                for command, body in {
                    "rustup": f"printf '{version}-x86_64-unknown-linux-gnu\\n'",
                    "cargo": "printf '%s' '" + json.dumps({"packages": packages}) + "'",
                }.items():
                    executable = temp / command
                    executable.write_text("#!/bin/sh\n" + body + "\n")
                    executable.chmod(0o755)
                result = subprocess.run(
                    [sys.executable, "scripts/architecture.py"],
                    cwd=ROOT,
                    env=dict(os.environ, PATH=str(temp)),
                    capture_output=True,
                    text=True,
                    timeout=10,
                )
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("unclassified workspace member", result.stderr)


if __name__ == "__main__":
    unittest.main()
