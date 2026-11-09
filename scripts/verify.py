"""Run real gates and retain current-run output, including cleanup failures."""

import os
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

from dependencies import up
from evidence import Evidence
from fixtures import PLAYWRIGHT
from lifecycle import (
    Containers,
    defer_termination,
    processes,
    report_cleanup,
    run_command,
    termination_signals,
)
from preflight import check

ROOT = Path(__file__).resolve().parents[1]
EVIDENCE = ROOT / "docs/evidence"
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
    BUN_INSTALL_CACHE_DIR=str(ROOT / ".local/bun-cache"),
)


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


def acceptance(prefix, evidence, containers, *, browser_timeout=180):
    with evidence.gate(prefix + "-ready"):
        ready()
    evidence.run(
        prefix + "-http", ["python3", "tests/http_acceptance.py"], env=environment
    )
    evidence.run(
        prefix + "-metrics",
        [str(ROOT / ".venv/bin/python"), "tests/metrics.py"],
        env=environment,
    )
    evidence.run(prefix + "-sdk", ["bun", "test"], cwd=ROOT / "sdk", env=environment)
    with evidence.gate(prefix + "-browser"):
        container = containers.create(
            prefix + "-browser",
            [
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
                PLAYWRIGHT,
                "node",
                "browser.mjs",
            ],
            mounts=[ROOT],
        )
        evidence.run(
            prefix + "-browser-exec",
            ["podman", "start", "--attach", container],
            env=environment,
            timeout=browser_timeout,
        )
        # Attached clients can return zero even when a container workload failed.
        result = run_command(
            ["podman", "inspect", "--format", "{{.State.ExitCode}}", container],
            stdout=subprocess.PIPE,
            text=True,
        )
        if result.stdout.strip() != "0":
            raise RuntimeError(f"Browser container exit: {result.stdout.strip()}")


def verify(evidence, containers):
    for script in [
        "tooling_lifecycle",
        "tooling_teardown",
        "tooling_launchers",
        "tooling_guards",
        "tooling_evidence",
        "tooling_browser",
    ]:
        evidence.run(script, ["python3", f"tests/{script}.py"], env=environment)
    evidence.run(
        "seed-conflicts", ["python3", "tests/seed_conflicts.py"], env=environment
    )
    evidence.run("seed", ["python3", "scripts/seed.py"], env=environment)
    evidence.run("seed-idempotency", ["python3", "scripts/seed.py"], env=environment)
    evidence.run(
        "sdk-install",
        ["bun", "install", "--frozen-lockfile"],
        cwd=ROOT / "sdk",
        env=environment,
    )
    evidence.run(
        "lint", ["python3", "scripts/lint.py", "check"], timeout=900, env=environment
    )
    evidence.run(
        "lint-negative-controls",
        ["python3", "tests/lint_policy.py"],
        timeout=300,
        env=environment,
    )
    evidence.run("bun-audit", ["bun", "audit"], cwd=ROOT / "sdk", env=environment)
    evidence.run(
        "architecture", ["python3", "scripts/architecture.py"], env=environment
    )
    evidence.run(
        "rust-tests",
        ["cargo", "test", "--locked", "--workspace"],
        timeout=300,
        env=environment,
    )
    evidence.run(
        "dev-lifecycle", ["python3", "tests/dev_lifecycle.py"], env=environment
    )
    evidence.run(
        "release",
        ["cargo", "build", "--locked", "--release", "--workspace"],
        timeout=300,
        env=environment,
    )
    with evidence.gate("contract-drift"):
        before = (
            (ROOT / "openapi.json").read_bytes(),
            (ROOT / "sdk/src/schema.d.ts").read_bytes(),
        )
        schema = run_command(
            ["target/release/cli", "openapi"], env=environment, stdout=subprocess.PIPE
        ).stdout
        (ROOT / "openapi.json").write_bytes(schema)
        evidence.run(
            "sdk-generate",
            ["bun", "run", "generate"],
            cwd=ROOT / "sdk",
            env=environment,
        )
        assert before == (schema, (ROOT / "sdk/src/schema.d.ts").read_bytes()), (
            "Generated contract drift"
        )
    evidence.run(
        "sdk-types", ["bun", "run", "check"], cwd=ROOT / "sdk", env=environment
    )
    with evidence.gate("native-lifecycle"), processes() as children:
        for name in ["server", "worker"]:
            with (EVIDENCE / f"native-{name}.log").open("w") as log:
                children.append(
                    subprocess.Popen(
                        [f"target/release/{name}"],
                        env=environment,
                        stdout=log,
                        stderr=subprocess.STDOUT,
                        start_new_session=True,
                    )
                )
        acceptance("native", evidence, containers)
    evidence.run(
        "native-observability",
        ["python3", "tests/observability.py", "native"],
        env=environment,
    )
    # This launcher owns cleanup: invoke its workflow in-process, never kill its owner on a gate timeout.
    with evidence.gate("revision-compatibility"):
        import runpy

        with (EVIDENCE / "revision-compatibility.log").open("w") as log:
            from contextlib import redirect_stderr, redirect_stdout

            with redirect_stdout(log), redirect_stderr(log):
                runpy.run_path(
                    str(ROOT / "tests/revision_compatibility.py"), run_name="__main__"
                )
    evidence.run(
        "oci-build",
        ["podman", "build", "-t", "localhost/rust-backend-template:test", "."],
        timeout=300,
        env=environment,
    )
    for name in ["server", "worker"]:
        arguments = [
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
            arguments.extend(["-e", key])
        arguments.extend(
            [
                "--entrypoint",
                f"/usr/local/bin/{name}",
                "localhost/rust-backend-template:test",
            ]
        )
        container = containers.create(name, arguments)
        containers.start(container)
    acceptance("oci", evidence, containers)
    for name in ["server", "worker"]:
        container = containers.resources[name]["id"]
        evidence.run("oci-" + name, ["podman", "logs", container], env=environment)
    evidence.run(
        "oci-observability",
        ["python3", "tests/observability.py", "oci"],
        env=environment,
    )


def main():
    os.chdir(ROOT)
    evidence = Evidence(EVIDENCE)
    owned_dependencies = not (ROOT / ".local/dependencies.json").exists()
    containers = Containers(
        ROOT / ".local" / f"verify-{evidence.run_id}.json", environment=environment
    )
    expected = [
        "preflight",
        "tooling_lifecycle",
        "tooling_teardown",
        "tooling_launchers",
        "tooling_guards",
        "tooling_evidence",
        "tooling_browser",
        "seed-conflicts",
        "seed",
        "seed-idempotency",
        "sdk-install",
        "lint",
        "lint-negative-controls",
        "bun-audit",
        "architecture",
        "rust-tests",
        "dev-lifecycle",
        "release",
        "contract-drift",
        "sdk-generate",
        "sdk-types",
        "native-lifecycle",
        "revision-compatibility",
        "oci-build",
        "oci-server",
        "oci-worker",
        "cleanup",
    ]
    for prefix in ["native", "oci"]:
        expected.extend(
            prefix + "-" + name
            for name in [
                "ready",
                "http",
                "metrics",
                "sdk",
                "browser",
                "browser-exec",
                "observability",
            ]
        )
    if owned_dependencies:
        expected.append("dependencies")
    with evidence.session(expected), termination_signals():
        try:
            with evidence.gate("preflight"):
                check()
                revision = run_command(
                    ["git", "rev-parse", "HEAD"],
                    capture_output=True,
                    text=True,
                    check=False,
                    timeout=10,
                )
                evidence.metadata["revision"] = (
                    revision.stdout.strip() if revision.returncode == 0 else None
                )
                status = run_command(
                    ["git", "status", "--porcelain"],
                    capture_output=True,
                    text=True,
                    check=False,
                    timeout=10,
                )
                evidence.metadata["dirty"] = (
                    bool(status.stdout) if status.returncode == 0 else None
                )
            if owned_dependencies:
                with evidence.gate("dependencies"):
                    up()
            verify(evidence, containers)
        finally:
            primary = sys.exception()
            try:
                with evidence.gate("cleanup"), defer_termination():
                    errors = containers.cleanup()
                    if owned_dependencies:
                        errors.extend(
                            Containers(ROOT / ".local/dependencies.json").cleanup()
                        )
                    if errors:
                        raise RuntimeError("Cleanup incomplete: " + "; ".join(errors))
            except Exception as error:
                if primary is None:
                    raise
                # The gate records cleanup failure without replacing the cause
                # that brought the workflow here (including cancellation).
                report_cleanup([str(error)], primary)


if __name__ == "__main__":
    main()
