"""Bounded subprocess cleanup and exact-ID disposable container ownership."""

import json
import os
import re
import signal
import subprocess
import sys
import uuid
from contextlib import contextmanager
from pathlib import Path


def atomic_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_text(json.dumps(value, indent=2) + "\n")
    temporary.replace(path)


def terminate(process, timeout=20):
    """Stop the owned session, including grandchildren, and reap its leader."""
    errors = []
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    try:
        process.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        errors.append(f"process {process.pid}: graceful shutdown timed out")
    # The leader may have exited while a grandchild retained the group.
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        errors.append(f"process {process.pid}: reap timed out")
    return errors


def run_command(
    command, *, timeout=180, check=True, input=None, capture_output=False, **kwargs
):
    """Like subprocess.run, but cancellation also stops the owned process group."""
    if input is not None:
        kwargs["stdin"] = subprocess.PIPE
    if capture_output:
        kwargs["stdout"] = subprocess.PIPE
        kwargs["stderr"] = subprocess.PIPE
    with subprocess.Popen(command, start_new_session=True, **kwargs) as process:
        try:
            stdout, stderr = process.communicate(input=input, timeout=timeout)
        except BaseException as error:
            for failure in terminate(process):
                error.add_note(failure)
            raise
        result = subprocess.CompletedProcess(
            command, process.returncode, stdout, stderr
        )
        if check:
            result.check_returncode()
        return result


@contextmanager
def termination_signals():
    """Translate INT/TERM to unwinding; a second signal cannot abort cleanup."""

    def stop(signum, frame):
        signal.signal(signal.SIGINT, signal.SIG_IGN)
        signal.signal(signal.SIGTERM, signal.SIG_IGN)
        raise SystemExit(128 + signum)

    previous = {
        sig: signal.signal(sig, stop) for sig in [signal.SIGINT, signal.SIGTERM]
    }
    try:
        yield
    finally:
        for sig, handler in previous.items():
            signal.signal(sig, handler)


@contextmanager
def defer_termination():
    """Finish bounded teardown before replaying INT/TERM to the outer owner.

    Nesting forwards cancellation to the enclosing shield, not past it. An
    already-unwinding failure remains primary; cancellation is attached to it.
    Callers must keep their teardown operations bounded.
    """
    primary = sys.exception()
    pending = None

    def defer(signum, frame):
        nonlocal pending
        if pending is None:
            pending = signum

    previous = {sig: signal.getsignal(sig) for sig in (signal.SIGINT, signal.SIGTERM)}
    for sig, handler in previous.items():
        if handler != signal.SIG_IGN:
            signal.signal(sig, defer)
    try:
        yield
    finally:
        for sig, handler in previous.items():
            signal.signal(sig, handler)
        if pending is not None:
            handler = previous[pending]
            if primary is not None:
                primary.add_note(
                    f"Termination signal {pending} deferred through cleanup"
                )
            elif callable(handler):
                handler(pending, None)
            else:
                raise SystemExit(128 + pending)


def report_cleanup(errors, primary):
    if not errors:
        return
    message = "Cleanup incomplete: " + "; ".join(errors)
    if primary is not None:
        primary.add_note(message)
        print(message, file=sys.stderr)
    else:
        raise RuntimeError(message)


@contextmanager
def processes(timeout=20):
    """Register Popen children started with start_new_session=True."""
    owned = []
    try:
        yield owned
    finally:
        primary = sys.exception()
        with defer_termination():
            errors = []
            for process in owned:
                try:
                    errors.extend(terminate(process, timeout=timeout))
                except Exception as error:
                    errors.append(str(error))
            report_cleanup(errors, primary)


class Containers:
    """Persist creation uncertainty before spawn; only exact IDs authorize rm.

    Mounted paths are retained as evidence, never deleted by this owner. A failed
    create cannot establish that the runtime did not allocate an anonymous volume.
    Legacy key-to-ID manifests remain readable; new manifests record obligations.
    """

    def __init__(self, state, *, executable="podman", environment=None):
        self.state = Path(state)
        self.executable = executable
        self.environment = environment
        self.resources = (
            json.loads(self.state.read_text()) if self.state.exists() else {}
        )
        for key, value in self.resources.items():
            if isinstance(value, str):
                self.resources[key] = {
                    "id": value,
                    "cidfile": None,
                    "mounts": [],
                    "status": "known",
                }

    def save(self):
        atomic_json(self.state, self.resources)

    def create(self, key, arguments, *, mounts=(), timeout=60):
        if key in self.resources:
            raise RuntimeError(f"Resource already recorded: {key}")
        cidfile = self.state.parent / f"{self.state.stem}-{uuid.uuid4().hex}.cid"
        record = {
            "id": None,
            "cidfile": str(cidfile),
            "mounts": [str(p) for p in mounts],
            "status": "unknown",
        }
        self.resources[key] = record
        self.save()
        try:
            result = run_command(
                [self.executable, "create", "--cidfile", str(cidfile), *arguments],
                env=self.environment,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                timeout=timeout,
            )
        except OSError:
            # Popen failed before there was a runtime client to allocate anything.
            del self.resources[key]
            self.save()
            raise
        finally:
            if key in self.resources:
                self.recover(record)
                self.save()
        if record["id"] is None:
            raise RuntimeError(
                f"Container allocation unknown for {key}; retain {self.state}; output: {result.stdout!r}"
            )
        return record["id"]

    @staticmethod
    def recover(record):
        if record["id"] is not None or not record["cidfile"]:
            return
        try:
            candidate = Path(record["cidfile"]).read_text().strip()
        except OSError:
            return
        if re.fullmatch(r"[a-f0-9]{64}", candidate):
            record["id"] = candidate
            record["status"] = "known"

    def start(self, container):
        return run_command(
            [self.executable, "start", container],
            env=self.environment,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=30,
        )

    @defer_termination()
    def cleanup(self):
        errors = []
        for key, record in list(self.resources.items()):
            try:
                self.recover(record)
                container = record["id"]
                if container is None or not re.fullmatch(r"[a-f0-9]{64}", container):
                    raise RuntimeError(
                        f"{key}: allocation unknown; retain cidfile/mount evidence in {self.state}"
                    )
                run_command(
                    [self.executable, "rm", "-f", "--volumes", container],
                    env=self.environment,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                    timeout=30,
                )
                if record["cidfile"]:
                    Path(record["cidfile"]).unlink(missing_ok=True)
                del self.resources[key]
            except Exception as error:
                errors.append(f"{key}: {error}")
            finally:
                self.save()
        if not self.resources:
            self.state.unlink(missing_ok=True)
        return errors

    def __enter__(self):
        return self

    def __exit__(self, kind, error, traceback):
        report_cleanup(self.cleanup(), error)


def container_id(state, key):
    record = Containers(state).resources[key]
    container = record["id"]
    if container is None or not re.fullmatch(r"[a-f0-9]{64}", container):
        raise RuntimeError(f"No proven container ID for {key} in {state}")
    return container
