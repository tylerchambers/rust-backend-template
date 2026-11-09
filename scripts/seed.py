"""Seed only the exact disposable PostgreSQL container owned by this checkout."""

import uuid
from pathlib import Path

from lifecycle import container_id, run_command, termination_signals

ROOT = Path(__file__).resolve().parents[1]


def seed(database="template"):
    container = container_id(ROOT / ".local/dependencies.json", "postgres")
    application = "seed_" + uuid.uuid4().hex
    command = [
        "podman",
        "exec",
        "-e",
        f"PGAPPNAME={application}",
        "-i",
        container,
        "psql",
        "-U",
        "postgres",
        "-d",
        database,
        "-v",
        "ON_ERROR_STOP=1",
    ]
    with termination_signals():
        try:
            run_command(command, input=(ROOT / "dev/seed.sql").read_bytes(), timeout=30)
        except BaseException as error:
            # Killing the podman exec client does not terminate its PostgreSQL backend.
            # This unique application name authorizes only this seed's connection.
            try:
                run_command(
                    [
                        "podman",
                        "exec",
                        container,
                        "psql",
                        "-U",
                        "postgres",
                        "-d",
                        database,
                        "-v",
                        "ON_ERROR_STOP=1",
                        "-c",
                        f"SELECT pg_terminate_backend(pid, 5000) FROM pg_stat_activity WHERE datname=current_database() AND application_name='{application}'",
                    ],
                    timeout=10,
                )
            except Exception as cleanup_error:
                error.add_note(f"Seed backend cleanup failed: {cleanup_error}")
            raise
    print(
        "Verified two seeded tenant projects; authenticate owner@example.test through the normal captured magic link"
    )


if __name__ == "__main__":
    seed()
