"""Run real gates and retain output; own only processes/containers we create."""

import json
import os
import subprocess
import time
import urllib.request
from pathlib import Path

from preflight import check

ROOT = Path(__file__).resolve().parents[1]
os.chdir(ROOT)

check()
EVIDENCE = ROOT / "docs/evidence"
EVIDENCE.mkdir(exist_ok=True)
environment = dict(
    os.environ,
    TEST_DATABASE_URL="postgres://postgres:local-template-test@127.0.0.1:55439/template",
    DATABASE_URL="postgres://template_app:local-app-test@127.0.0.1:55439/template",
    PUBLIC_ORIGIN="http://localhost:58039",
    BIND_ADDRESS="127.0.0.1:58039",
    MAIL_MODE="capture",
    SMTP_PORT="51029",
    ENVIRONMENT="development",
    SQLX_OFFLINE="true",
)
environment["BUN_INSTALL_CACHE_DIR"] = str(ROOT / ".local/bun-cache")
results = []


def run(name, command, cwd=ROOT, timeout=180):
    started = time.monotonic()
    result = subprocess.run(
        command,
        cwd=cwd,
        env=environment,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=timeout,
    )
    (EVIDENCE / f"{name}.log").write_text(result.stdout)
    results.append(
        {
            "gate": name,
            "command": command,
            "exit_code": result.returncode,
            "seconds": round(time.monotonic() - started, 2),
        }
    )
    (EVIDENCE / "results.json").write_text(json.dumps(results, indent=2) + "\n")
    print(name, result.returncode, flush=True)
    if result.returncode:
        raise RuntimeError(f"{name}: see docs/evidence/{name}.log")


def ready():
    for _ in range(100):
        try:
            with urllib.request.urlopen(
                "http://127.0.0.1:58039/ready", timeout=2
            ) as response:
                if response.status == 200:
                    return
        except OSError:
            pass
        time.sleep(0.1)
    raise RuntimeError("Server did not become ready")


def acceptance(prefix):
    ready()
    run(prefix + "-http", ["python3", "tests/http_acceptance.py"])
    run(prefix + "-sdk", ["bun", "test"], ROOT / "sdk")
    run(
        prefix + "-browser",
        [
            "podman",
            "run",
            "--rm",
            "--network",
            "host",
            "--ipc",
            "private",
            "--security-opt",
            "no-new-privileges",
            "-v",
            f"{ROOT}:/app:ro",
            "-w",
            "/app/sdk",
            "mcr.microsoft.com/playwright:v1.61.1-noble@sha256:5b8f294aff9041b7191c34a4bab3ac270157a28774d4b0660e9743297b697e48",
            "node",
            "browser.mjs",
        ],
    )


owned_dependencies = not (ROOT / ".local/dependencies.json").exists()
try:
    if owned_dependencies:
        run("dependencies", ["python3", "scripts/dependencies.py", "up"])
    run("seed", ["python3", "scripts/seed.py"])
    run("seed-idempotency", ["python3", "scripts/seed.py"])
    run("sdk-install", ["bun", "install", "--frozen-lockfile"], ROOT / "sdk")
    run("lint", ["python3", "scripts/lint.py", "check"], timeout=900)
    run("lint-negative-controls", ["python3", "tests/lint_policy.py"], timeout=300)
    run("bun-audit", ["bun", "audit"], ROOT / "sdk")
    run("architecture", ["python3", "scripts/architecture.py"])
    run("rust-tests", ["cargo", "test", "--locked", "--workspace"])
    run("dev-lifecycle", ["python3", "tests/dev_lifecycle.py"])
    run("release", ["cargo", "build", "--locked", "--release", "--workspace"])
    before = (
        (ROOT / "openapi.json").read_bytes(),
        (ROOT / "sdk/src/schema.d.ts").read_bytes(),
    )
    schema = subprocess.check_output(["target/release/cli", "openapi"], env=environment)
    (ROOT / "openapi.json").write_bytes(schema)
    run("sdk-generate", ["bun", "run", "generate"], ROOT / "sdk")
    assert before == (schema, (ROOT / "sdk/src/schema.d.ts").read_bytes()), (
        "Generated contract drift"
    )
    run("sdk-types", ["bun", "run", "check"], ROOT / "sdk")
    processes = []
    try:
        for name in ["server", "worker"]:
            with (EVIDENCE / f"native-{name}.log").open("w") as log:
                processes.append(
                    subprocess.Popen(
                        [f"target/release/{name}"],
                        env=environment,
                        stdout=log,
                        stderr=subprocess.STDOUT,
                    )
                )
        acceptance("native")
    finally:
        for process in processes:
            process.terminate()
        for process in processes:
            try:
                process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
                raise RuntimeError(
                    "Native graceful shutdown exceeded deadline"
                ) from None
    run("native-observability", ["python3", "tests/observability.py", "native"])
    run(
        "revision-compatibility",
        ["python3", "tests/revision_compatibility.py"],
        timeout=600,
    )
    run(
        "oci-build",
        ["podman", "build", "-t", "localhost/rust-backend-template:test", "."],
    )
    containers = []
    try:
        for name in ["server", "worker"]:
            command = [
                "podman",
                "run",
                "-d",
                "--network",
                "host",
                "--read-only",
                "--cap-drop",
                "all",
                "--security-opt",
                "no-new-privileges",
            ]
            for key in [
                "DATABASE_URL",
                "PUBLIC_ORIGIN",
                "BIND_ADDRESS",
                "MAIL_MODE",
                "SMTP_PORT",
                "ENVIRONMENT",
            ]:
                command.extend(["-e", key])
            command.extend(
                [
                    "--entrypoint",
                    f"/usr/local/bin/{name}",
                    "localhost/rust-backend-template:test",
                ]
            )
            containers.append(
                subprocess.check_output(command, env=environment, text=True).strip()
            )
        acceptance("oci")
        for name, container in zip(["server", "worker"], containers, strict=True):
            output = subprocess.run(
                ["podman", "logs", container],
                check=True,
                capture_output=True,
                text=True,
            )
            (EVIDENCE / f"oci-{name}.log").write_text(output.stdout + output.stderr)
        run("oci-observability", ["python3", "tests/observability.py", "oci"])
    finally:
        for container in containers:
            subprocess.run(
                ["podman", "stop", "--time", "15", container],
                check=True,
                capture_output=True,
            )
            subprocess.run(["podman", "rm", container], check=True, capture_output=True)
finally:
    if owned_dependencies:
        subprocess.run(["python3", "scripts/dependencies.py", "down"], check=True)
