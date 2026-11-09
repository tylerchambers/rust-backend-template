"""Run the API and worker with loopback-only, externally captured email."""

import os
import subprocess
import sys
import time
from pathlib import Path

from dependencies import up
from lifecycle import (
    Containers,
    processes,
    report_cleanup,
    run_command,
    termination_signals,
)
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


def main():
    owned_dependencies = not (ROOT / ".local/dependencies.json").exists()
    with termination_signals():
        try:
            # Keep the cleanup owner in this process, not under a child kill deadline.
            if owned_dependencies:
                up()
            with processes() as children:
                run_command(["cargo", "build", "--locked", "--workspace"], timeout=300)
                for name in ["server", "worker"]:
                    children.append(
                        subprocess.Popen(
                            [f"target/debug/{name}"],
                            env=environment,
                            start_new_session=True,
                        )
                    )
                print(
                    "API: http://localhost:58039 | email capture: http://localhost:58029 | Ctrl-C stops owned processes",
                    flush=True,
                )
                while all(child.poll() is None for child in children):
                    time.sleep(0.2)
                raise RuntimeError("API or worker exited unexpectedly")
        finally:
            if owned_dependencies:
                report_cleanup(
                    Containers(ROOT / ".local/dependencies.json").cleanup(),
                    sys.exception(),
                )


if __name__ == "__main__":
    main()
