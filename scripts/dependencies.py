"""Start disposable dependencies; retain exact ownership until removal succeeds."""

import os
import subprocess
import sys
import time
import urllib.request
import uuid
from pathlib import Path

from fixtures import dependency_fixtures
from lifecycle import Containers, report_cleanup, run_command, termination_signals
from preflight import check

ROOT = Path(__file__).resolve().parents[1]
STATE = ROOT / ".local/dependencies.json"


def up():
    if STATE.exists():
        raise RuntimeError("Dependencies already recorded; use down before a fresh up")
    check()
    tls = ROOT / ".local/smtp"
    tls.mkdir(parents=True, exist_ok=True)
    run_command(
        [
            "openssl",
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-keyout",
            str(tls / "key.pem"),
            "-out",
            str(tls / "cert.pem"),
            "-days",
            "2",
            "-subj",
            "/CN=localhost",
            "-addext",
            "subjectAltName=DNS:localhost",
            "-addext",
            "basicConstraints=critical,CA:FALSE",
            "-addext",
            "extendedKeyUsage=serverAuth",
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=15,
    )
    owner = Containers(STATE)
    try:
        containers = {}
        run = uuid.uuid4().hex[:12]
        for key, image, arguments in dependency_fixtures(tls):
            container = owner.create(
                key,
                [
                    "--name",
                    f"rbt-{run}-{key}",
                    "--label",
                    "project=rust-backend-template",
                    "--network",
                    "slirp4netns",
                    *arguments,
                    image,
                ],
                mounts=[tls] if key == "mail" else [],
            )
            containers[key] = container
            owner.start(container)
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            ready = run_command(
                [
                    "podman",
                    "exec",
                    containers["postgres"],
                    "pg_isready",
                    "-h",
                    "127.0.0.1",
                    "-U",
                    "postgres",
                ],
                check=False,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=5,
            )
            if ready.returncode == 0:
                break
            time.sleep(0.2)
        else:
            raise RuntimeError("Postgres readiness deadline exceeded")
        run_command(
            ["cargo", "run", "--locked", "-p", "cli", "--", "migrate"],
            cwd=ROOT,
            env=dict(
                os.environ,
                DATABASE_URL="postgres://postgres:local-template-test@127.0.0.1:55439/template",
            ),
        )
        sql = [
            "podman",
            "exec",
            containers["postgres"],
            "psql",
            "-U",
            "postgres",
            "-d",
            "template",
            "-v",
            "ON_ERROR_STOP=1",
        ]
        run_command(
            sql + ["-c", "ALTER ROLE template_app LOGIN PASSWORD 'local-app-test';"],
            timeout=15,
        )
        privilege = run_command(
            sql
            + [
                "-Atc",
                "SELECT has_table_privilege('template_app','river_job','INSERT')",
            ],
            stdout=subprocess.PIPE,
            text=True,
            timeout=15,
        )
        assert privilege.stdout.strip() == "t", (
            "Fresh runtime role cannot enqueue River jobs"
        )
        with urllib.request.urlopen(
            "http://127.0.0.1:58029/api/v1/info", timeout=5
        ) as response:
            assert response.status == 200
    except BaseException as error:
        report_cleanup(owner.cleanup(), error)
        raise
    print(
        "Disposable PostgreSQL, SMTP capture and Redis ready; runtime role provisioned"
    )


def main():
    with termination_signals():
        if sys.argv[1:] == ["down"]:
            report_cleanup(Containers(STATE).cleanup(), None)
        elif sys.argv[1:] == ["up"]:
            up()
        else:
            raise SystemExit("usage: dependencies.py up|down")


if __name__ == "__main__":
    main()
