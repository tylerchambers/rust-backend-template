"""Upgrade a prior schema with live data, then exercise two real app revisions."""

import io
import json
import os
import re
import shutil
import subprocess
import tarfile
import time
import urllib.request
import uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PREVIOUS = "1b266ba502ae8e207a667aa27681913264341ec6"
source = ROOT / ".local/previous-revision"
source.mkdir(parents=True, exist_ok=True)
archive = subprocess.check_output(["git", "archive", PREVIOUS], cwd=ROOT)
with tarfile.open(fileobj=io.BytesIO(archive)) as files:
    files.extractall(source, filter="data")
# Original queries are unchanged; metadata is build input, not an edited old source.
shutil.copytree(ROOT / ".sqlx", source / ".sqlx", dirs_exist_ok=True)
env = dict(os.environ, SQLX_OFFLINE="true")
env["CARGO_TARGET_DIR"] = str(ROOT / ".local/previous-target")
# Same-name path packages from two revisions must not share Cargo fingerprints.
subprocess.run(
    ["cargo", "build", "--locked", "--release", "-p", "server", "-p", "cli"],
    cwd=source,
    env=env,
    check=True,
    timeout=300,
)
old_binary = ROOT / ".local/previous-target/release/server"
old_cli = ROOT / ".local/previous-target/release/cli"
env["CARGO_TARGET_DIR"] = str(ROOT / "target")
subprocess.run(
    ["cargo", "build", "--locked", "--release", "-p", "server", "-p", "cli"],
    cwd=ROOT,
    env=env,
    check=True,
    timeout=300,
)

cookies = {}
processes = []


def request(port, path, body=None):
    origin = f"http://localhost:{port}"
    headers = {
        "Origin": origin,
        "Content-Type": "application/json",
        "Cookie": "; ".join(f"{k}={v}" for k, v in cookies.items()),
    }
    if "csrf" in cookies:
        headers["x-csrf-token"] = cookies["csrf"]
    req = urllib.request.Request(
        origin + path,
        data=None if body is None else json.dumps(body).encode(),
        headers=headers,
    )
    with urllib.request.urlopen(req, timeout=10) as response:
        for value in response.headers.get_all("Set-Cookie", []):
            key, value = value.split(";", 1)[0].split("=", 1)
            cookies[key] = value
        content = response.read()
        if response.headers.get("Content-Type", "").startswith("application/json"):
            return json.loads(content)
        return None


def start_server(port, binary):
    process_env = dict(
        env, PUBLIC_ORIGIN=f"http://localhost:{port}", BIND_ADDRESS=f"127.0.0.1:{port}"
    )
    with (ROOT / f"docs/evidence/compat-{port}.log").open("w") as log:
        process = subprocess.Popen(
            [str(binary)], env=process_env, stdout=log, stderr=subprocess.STDOUT
        )
    processes.append(process)
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        try:
            request(port, "/health")
            return
        except OSError:
            if process.poll() is not None:
                raise RuntimeError(
                    "Compatibility server exited; inspect its log"
                ) from None
            time.sleep(0.05)
    raise RuntimeError("Compatibility readiness deadline exceeded")


container = None
try:
    created = subprocess.run(
        [
            "podman",
            "create",
            "--network",
            "slirp4netns",
            "-p",
            "127.0.0.1::5432",
            "-e",
            "POSTGRES_PASSWORD=compat-test",
            "-e",
            "POSTGRES_DB=template",
            "docker.io/library/postgres:17-alpine@sha256:18cfe3ef5e6815560c98237d6216d1e5119702fb0f3894c8785dd58b8bbe5d73",
        ],
        capture_output=True,
        text=True,
        timeout=60,
    )
    candidate = created.stdout.strip()
    if created.returncode or not re.fullmatch(r"[a-f0-9]{64}", candidate):
        raise RuntimeError(
            "Compatibility PostgreSQL creation ownership uncertain; no name-based cleanup attempted"
        )
    container = candidate
    subprocess.run(
        ["podman", "start", container], check=True, capture_output=True, timeout=30
    )
    address = subprocess.check_output(
        ["podman", "port", container, "5432"], text=True, timeout=10
    ).strip()
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        ready = subprocess.run(
            [
                "podman",
                "exec",
                container,
                "pg_isready",
                "-h",
                "127.0.0.1",
                "-U",
                "postgres",
            ],
            capture_output=True,
            timeout=5,
        )
        if ready.returncode == 0:
            break
        time.sleep(0.1)
    else:
        raise RuntimeError("Compatibility PostgreSQL readiness deadline exceeded")
    deploy = dict(
        env, DATABASE_URL=f"postgres://postgres:compat-test@{address}/template"
    )
    subprocess.run([str(old_cli), "migrate"], env=deploy, check=True, timeout=30)
    subprocess.run(
        [
            "podman",
            "exec",
            container,
            "psql",
            "-U",
            "postgres",
            "-d",
            "template",
            "-v",
            "ON_ERROR_STOP=1",
            "-c",
            "ALTER ROLE template_app LOGIN PASSWORD 'compat-app';",
        ],
        check=True,
        capture_output=True,
        timeout=10,
    )
    env["DATABASE_URL"] = f"postgres://template_app:compat-app@{address}/template"
    start_server(58041, old_binary)
    request(58041, "/v1/auth/confirmation")
    email = f"compat-{uuid.uuid4()}@example.test"
    request(58041, "/v1/auth/request", {"email": email})
    with urllib.request.urlopen(
        "http://127.0.0.1:58029/api/v1/messages", timeout=5
    ) as response:
        messages = json.load(response)["messages"]
    message = next(m for m in messages if m["To"][0]["Address"] == email)
    with urllib.request.urlopen(
        "http://127.0.0.1:58029/api/v1/message/" + message["ID"], timeout=5
    ) as response:
        match = re.search(r"confirmation#([a-f0-9]{64})", json.load(response)["Text"])
    assert match is not None
    identity = request(58041, "/v1/auth/confirm", {"token": match.group(1)})
    organization = request(
        58041, "/v1/organizations", {"name": "Created before upgrade"}
    )
    assert organization is not None
    org = organization["id"]
    # Old schema contains a real identity, session, organization, membership and audit row.
    # Migration 7 also repairs the previous revision's missing runtime River grants.
    subprocess.run(
        [str(ROOT / "target/release/cli"), "migrate"],
        env=deploy,
        check=True,
        timeout=30,
    )
    start_server(58042, ROOT / "target/release/server")
    sessions = request(58042, "/v1/auth/sessions")
    assert isinstance(sessions, list) and len(sessions) == 1
    path = f"/v1/organizations/{org}/projects"
    old = request(58041, path, {"name": "Written by previous revision"})
    assert request(58042, path) == [old]
    new = request(58042, path, {"name": "Written by current revision"})
    assert request(58041, path) == [old, new]
    print(
        f"PASS: upgrade from {PREVIOUS} preserves live identity/session/tenant data; old and current binaries concurrently read/write expanded schema"
    )
finally:
    for process in processes:
        process.terminate()
    for process in processes:
        try:
            process.wait(timeout=20)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
    if container is not None:
        subprocess.run(
            ["podman", "rm", "-f", "--volumes", container],
            check=True,
            capture_output=True,
            timeout=30,
        )
