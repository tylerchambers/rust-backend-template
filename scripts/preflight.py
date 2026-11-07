"""Refuse Cargo's implicit rustup installation before starting local workflows."""
from pathlib import Path
import shutil
import subprocess
import tomllib


def check():
    root = Path(__file__).resolve().parents[1]
    version = tomllib.loads((root / "rust-toolchain.toml").read_text())["toolchain"]["channel"]
    for command in ["rustup", "cargo", "bun", "podman", "openssl"]:
        if shutil.which(command) is None:
            raise SystemExit(f"Required preinstalled command missing: {command}; no host installation attempted")
    installed = subprocess.check_output(["rustup", "toolchain", "list"], text=True, timeout=10)
    if not any(line.startswith(version + "-") for line in installed.splitlines()):
        raise SystemExit(f"Rust {version} must already be installed; refusing implicit rustup installation")


if __name__ == "__main__":
    check()
