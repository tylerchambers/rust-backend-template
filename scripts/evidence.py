"""An atomic current-run ledger; logs stream to disk even on interruption."""

import subprocess
import time
import uuid
from contextlib import contextmanager
from datetime import UTC, datetime

from lifecycle import atomic_json, run_command


class Evidence:
    def __init__(self, directory):
        self.directory = directory
        directory.mkdir(parents=True, exist_ok=True)
        self.run_id = uuid.uuid4().hex
        self.results = []
        self.metadata = {
            "run_id": self.run_id,
            "status": "running",
            "started_at": datetime.now(UTC).isoformat(),
        }
        atomic_json(directory / "run.json", self.metadata)
        atomic_json(directory / "results.json", self.results)

    @contextmanager
    def session(self, expected):
        self.metadata["expected_gates"] = expected
        atomic_json(self.directory / "run.json", self.metadata)
        try:
            yield
            passed = {
                record["gate"]
                for record in self.results
                if record["status"] == "passed"
            }
            missing = set(expected) - passed
            if missing or any(record["status"] != "passed" for record in self.results):
                raise RuntimeError(
                    f"Verification incomplete; missing successful gates: {sorted(missing)}"
                )
        except BaseException as error:
            self.metadata["status"] = "failed"
            self.metadata["error"] = str(error)
            raise
        else:
            self.metadata["status"] = "completed"
        finally:
            self.metadata["finished_at"] = datetime.now(UTC).isoformat()
            atomic_json(self.directory / "run.json", self.metadata)

    @contextmanager
    def gate(self, name, command=None):
        record = {
            "run_id": self.run_id,
            "gate": name,
            "command": command,
            "status": "running",
            "exit_code": None,
        }
        self.results.append(record)
        atomic_json(self.directory / "results.json", self.results)
        started = time.monotonic()
        try:
            yield
        except BaseException as error:
            record["status"] = "failed"
            record["exit_code"] = 1
            if isinstance(error, subprocess.TimeoutExpired):
                record["exit_code"] = 124
            elif isinstance(error, subprocess.CalledProcessError):
                record["exit_code"] = error.returncode
            elif isinstance(error, SystemExit) and isinstance(error.code, int):
                record["exit_code"] = error.code
            record["error"] = str(error)
            raise
        else:
            record["status"] = "passed"
            record["exit_code"] = 0
        finally:
            record["seconds"] = round(time.monotonic() - started, 2)
            atomic_json(self.directory / "results.json", self.results)
            print(name, record["exit_code"], flush=True)

    def run(self, name, command, *, timeout=180, **kwargs):
        with (
            self.gate(name, command),
            (self.directory / f"{name}.log").open("w") as log,
        ):
            try:
                run_command(
                    command,
                    stdout=log,
                    stderr=subprocess.STDOUT,
                    timeout=timeout,
                    **kwargs,
                )
            except BaseException as error:
                log.write(f"\nFAILED: {error}\n")
                raise
            finally:
                log.flush()
