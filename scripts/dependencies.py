"""Own only container IDs created by this checkout; never prune by name."""

import json
import os
import subprocess
import sys
import time
import urllib.request
import uuid
from pathlib import Path

from preflight import check

root = Path(__file__).resolve().parents[1]
state = root / ".local/dependencies.json"
state.parent.mkdir(exist_ok=True)

if sys.argv[1:] == ["down"]:
    if state.exists():
        containers = json.loads(state.read_text())
        for key, container in list(containers.items()):
            subprocess.run(
                ["podman", "rm", "-f", "--volumes", container], check=True, timeout=30
            )
            del containers[key]
            state.write_text(json.dumps(containers))
        state.unlink()
    sys.exit(0)

if sys.argv[1:] != ["up"]:
    raise SystemExit("usage: dependencies.py up|down")
if state.exists():
    raise SystemExit("Dependencies already recorded; use down before a fresh up")


check()

tls = root / ".local/smtp"
tls.mkdir(exist_ok=True)
subprocess.run(
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
    check=True,
    capture_output=True,
    timeout=15,
)
run = uuid.uuid4().hex[:12]
containers = {}
state.write_text("{}")
fixtures = [
    (
        "postgres",
        "docker.io/library/postgres:17-alpine@sha256:18cfe3ef5e6815560c98237d6216d1e5119702fb0f3894c8785dd58b8bbe5d73",
        [
            "-p",
            "127.0.0.1:55439:5432",
            "-e",
            "POSTGRES_PASSWORD=local-template-test",
            "-e",
            "POSTGRES_DB=template",
        ],
    ),
    (
        "mail",
        "docker.io/axllent/mailpit:v1.27.8@sha256:6abc8e633df15eaf785cfcf38bae48e66f64beecdc03121e249d0f9ec15f0707",
        [
            "-p",
            "127.0.0.1:58029:8025",
            "-p",
            "127.0.0.1:51029:1025",
            "-e",
            "MP_DISABLE_VERSION_CHECK=true",
            "-v",
            f"{tls}:/tls:ro",
            "-e",
            "MP_SMTP_TLS_CERT=/tls/cert.pem",
            "-e",
            "MP_SMTP_TLS_KEY=/tls/key.pem",
            "-e",
            "MP_SMTP_AUTH_ACCEPT_ANY=true",
        ],
    ),
    (
        "redis",
        "docker.io/library/redis:7.4.2-alpine@sha256:02419de7eddf55aa5bcf49efb74e88fa8d931b4d77c07eff8a6b2144472b6952",
        ["-p", "127.0.0.1:56379:6379"],
    ),
]
for key, image, arguments in fixtures:
    result = subprocess.run(
        [
            "podman",
            "create",
            "--name",
            f"rbt-{run}-{key}",
            "--label",
            "project=rust-backend-template",
            "--network",
            "slirp4netns",
            *arguments,
            image,
        ],
        capture_output=True,
        text=True,
        timeout=60,
    )
    container = result.stdout.strip()
    if (
        result.returncode
        or len(container) != 64
        or any(c not in "0123456789abcdef" for c in container)
    ):
        raise SystemExit(
            f"Container creation ownership uncertain for {key}; inspect rbt-{run}-{key}. Recorded resources remain in {state}."
        )
    containers[key] = container
    state.write_text(json.dumps(containers))
    subprocess.run(["podman", "start", container], check=True, timeout=30)

deadline = time.monotonic() + 30
while time.monotonic() < deadline:
    ready = subprocess.run(
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
        capture_output=True,
    )
    if ready.returncode == 0:
        break
    time.sleep(0.2)
else:
    raise SystemExit("Postgres did not become ready; use dependencies.py down")
subprocess.run(
    ["cargo", "run", "--locked", "-p", "cli", "--", "migrate"],
    cwd=root,
    env={
        **os.environ,
        "DATABASE_URL": "postgres://postgres:local-template-test@127.0.0.1:55439/template",
    },
    check=True,
    timeout=180,
)
subprocess.run(
    [
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
        "-c",
        "ALTER ROLE template_app LOGIN PASSWORD 'local-app-test';",
    ],
    check=True,
    timeout=15,
)
privilege = subprocess.run(
    [
        "podman",
        "exec",
        containers["postgres"],
        "psql",
        "-U",
        "postgres",
        "-d",
        "template",
        "-Atc",
        "SELECT has_table_privilege('template_app','river_job','INSERT')",
    ],
    capture_output=True,
    text=True,
    check=True,
    timeout=15,
)
assert privilege.stdout.strip() == "t", "Fresh runtime role cannot enqueue River jobs"
with urllib.request.urlopen(
    "http://127.0.0.1:58029/api/v1/info", timeout=5
) as response:
    assert response.status == 200
print("Disposable PostgreSQL, SMTP capture and Redis ready; runtime role provisioned")
