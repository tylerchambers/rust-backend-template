"""Pinned, checkout-scoped quality tools; no service or system installation."""

import argparse
import json
import os
import subprocess
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DENY_VERSION = "0.20.2"
BIOME = ROOT / "sdk/node_modules/@biomejs/biome/bin/biome"


def run(command, cwd=ROOT):
    print("+", " ".join(map(str, command)), flush=True)
    subprocess.run(command, cwd=cwd, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["check", "fmt"], default="check", nargs="?")
    args = parser.parse_args()
    os.environ["UV_CACHE_DIR"] = str(ROOT / ".local/uv-cache")
    os.environ["UV_PYTHON_DOWNLOADS"] = "never"
    os.environ["BUN_INSTALL_CACHE_DIR"] = str(ROOT / ".local/bun-cache")
    expected_bun = json.loads((ROOT / "sdk/package.json").read_text())["packageManager"]
    bun = subprocess.check_output(["bun", "--version"], text=True).strip()
    if expected_bun != f"bun@{bun}":
        raise SystemExit(f"Expected {expected_bun}, found bun@{bun}")
    run(["uv", "sync", "--locked"])
    run(["bun", "install", "--frozen-lockfile"], ROOT / "sdk")
    ruff = [str(ROOT / ".venv/bin/ruff")]
    biome = ["bun", str(BIOME), "check", "--config-path", str(ROOT)]
    if args.action == "fmt":
        run(["cargo", "fmt", "--all"])
        run([*ruff, "check", "--fix", "scripts", "tests"])
        run([*ruff, "format", "scripts", "tests"])
        run([*biome, "--write", "--error-on-warnings", "sdk"])
        return

    metadata = json.loads(
        subprocess.check_output(
            ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"],
            cwd=ROOT,
            text=True,
        )
    )
    for package in metadata["packages"]:
        if package["id"] in metadata["workspace_members"]:
            manifest = tomllib.loads(Path(package["manifest_path"]).read_text())
            if manifest.get("lints") != {"workspace": True}:
                raise SystemExit(f"{package['name']} must inherit workspace lints")
    run(["cargo", "fmt", "--all", "--check"])
    run([*ruff, "format", "--check", "scripts", "tests"])
    run([*ruff, "check", "scripts", "tests"])
    run([*biome, "--error-on-warnings", "sdk"])
    run(
        [
            "cargo",
            "clippy",
            "--locked",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ]
    )
    deny = ROOT / ".local/tools/bin/cargo-deny"
    version = (
        subprocess.check_output([deny, "--version"], text=True).strip()
        if deny.exists()
        else ""
    )
    if version != f"cargo-deny {DENY_VERSION}":
        run(
            [
                "cargo",
                "install",
                "cargo-deny",
                "--version",
                DENY_VERSION,
                "--locked",
                "--root",
                str(ROOT / ".local/tools"),
            ]
        )
    run([str(deny), "--locked", "check", "--deny", "warnings"])


if __name__ == "__main__":
    os.chdir(ROOT)
    main()
