"""Run the API and worker with loopback-only, externally captured email."""

import os
import signal
import subprocess
import time
from pathlib import Path

from preflight import check

ROOT = Path(__file__).resolve().parents[1]
os.chdir(ROOT)

check()
environment = dict(
    os.environ,
    DATABASE_URL="postgres://template_app:local-app-test@127.0.0.1:55439/template",
    PUBLIC_ORIGIN="http://localhost:58039",
    BIND_ADDRESS="127.0.0.1:58039",
    MAIL_MODE="capture",
    SMTP_PORT="51029",
    ENVIRONMENT="development",
)
owned_dependencies = not (ROOT / ".local/dependencies.json").exists()
processes = []


def stop(signum, frame):
    raise SystemExit(128 + signum)


signal.signal(signal.SIGTERM, stop)
signal.signal(signal.SIGINT, stop)
try:
    if owned_dependencies:
        subprocess.run(["python3", "scripts/dependencies.py", "up"], check=True)
    subprocess.run(
        ["cargo", "build", "--locked", "--workspace"], check=True, timeout=300
    )
    for name in ["server", "worker"]:
        processes.append(subprocess.Popen([f"target/debug/{name}"], env=environment))
    print(
        "API: http://localhost:58039 | email capture: http://localhost:58029 | Ctrl-C stops owned processes",
        flush=True,
    )
    while all(process.poll() is None for process in processes):
        time.sleep(0.2)
    raise RuntimeError("API or worker exited unexpectedly")
finally:
    for process in processes:
        if process.poll() is None:
            process.terminate()
    for process in processes:
        try:
            process.wait(timeout=20)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
    if owned_dependencies:
        subprocess.run(
            ["python3", "scripts/dependencies.py", "down"], check=True, timeout=120
        )
