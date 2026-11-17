"""Real PostgreSQL conflicts must leave every fixture table unchanged."""

import json
import signal
import subprocess
import sys
import time
import unittest
import uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
from lifecycle import container_id, run_command  # noqa: E402


class TestSeed(unittest.TestCase):
    def sql(self, sql, database=None):
        return run_command(
            [
                "podman",
                "exec",
                "-i",
                self.container,
                "psql",
                "-U",
                "postgres",
                "-d",
                database or self.database,
                "-v",
                "ON_ERROR_STOP=1",
                "-At",
            ],
            input=sql,
            text=True,
            capture_output=True,
            timeout=15,
        ).stdout

    def setUp(self):
        self.container = container_id(ROOT / ".local/dependencies.json", "postgres")
        self.database = "seed_" + uuid.uuid4().hex
        self.sql(f"CREATE DATABASE {self.database} TEMPLATE template", "postgres")
        self.sql("TRUNCATE users, organizations CASCADE")
        self.sql(
            f"INSERT INTO users(id,email) VALUES ('{uuid.uuid4()}','unrelated@example.test'); INSERT INTO organizations(id,name) VALUES ('{uuid.uuid4()}','Unrelated tenant')"
        )

    def tearDown(self):
        self.sql(f"DROP DATABASE {self.database} WITH (FORCE)", "postgres")

    def seed(self):
        return run_command(
            [
                sys.executable,
                "-c",
                f"import sys; sys.path.insert(0, 'scripts'); from seed import seed; seed({self.database!r})",
            ],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
            timeout=40,
        )

    def snapshot(self):
        return [
            json.loads(
                self.sql(
                    f"SELECT coalesce(json_agg(t ORDER BY row_to_json(t)::text),'[]') FROM {table} t"
                )
            )
            for table in ["users", "organizations", "memberships", "projects"]
        ]

    def test_partial_fixture_conflict_rolls_back_new_ownership(self):
        self.sql(
            "INSERT INTO users(id,email) VALUES ('00000000-0000-7000-8000-000000000001','conflict@example.test')"
        )
        before = self.snapshot()
        self.assertNotEqual(self.seed().returncode, 0)
        self.assertEqual(self.snapshot(), before)

    def test_signal_during_insert_terminates_backend_and_rolls_back(self):
        self.sql(
            "CREATE FUNCTION delay_seed() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_sleep(15); RETURN NEW; END $$; CREATE TRIGGER delay_seed BEFORE INSERT ON users FOR EACH ROW EXECUTE FUNCTION delay_seed();"
        )
        before = self.snapshot()
        for sig in [signal.SIGINT, signal.SIGTERM]:
            with self.subTest(signal=sig):
                process = subprocess.Popen(
                    [
                        sys.executable,
                        "-c",
                        f"import sys; sys.path.insert(0, 'scripts'); from seed import seed; seed({self.database!r})",
                    ],
                    cwd=ROOT,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                    text=True,
                    start_new_session=True,
                )
                try:
                    deadline = time.monotonic() + 8
                    while time.monotonic() < deadline:
                        active = self.sql(
                            "SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND wait_event='PgSleep' AND query LIKE 'INSERT INTO users%'"
                        ).strip()
                        if active == "1":
                            break
                        self.assertIsNone(process.poll())
                        time.sleep(0.02)
                    else:
                        self.fail("Seed INSERT never reached trigger")
                    process.send_signal(sig)
                    process.communicate(timeout=8)
                    self.assertNotEqual(process.returncode, 0)
                    self.assertEqual(
                        self.sql(
                            "SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND pid<>pg_backend_pid() AND query LIKE 'INSERT INTO users%'"
                        ).strip(),
                        "0",
                    )
                    self.assertEqual(self.snapshot(), before)
                finally:
                    if process.poll() is None:
                        process.kill()
                        process.communicate()
                    self.sql(
                        "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname=current_database() AND pid<>pg_backend_pid()"
                    )

    def test_rerun_and_conflicts_are_atomic(self):
        self.assertEqual(self.seed().returncode, 0)
        baseline = self.snapshot()
        self.assertEqual(self.seed().returncode, 0)
        self.assertEqual(self.snapshot(), baseline)
        cases = [
            (
                "users",
                "email",
                "other@example.test",
                "id='00000000-0000-7000-8000-000000000001'",
            ),
            (
                "organizations",
                "name",
                "Other",
                "id='00000000-0000-7000-8000-000000000101'",
            ),
            ("projects", "name", "Other", "id='00000000-0000-7000-8000-000000001001'"),
            (
                "memberships",
                "role",
                "member",
                "organization_id='00000000-0000-7000-8000-000000000101'",
            ),
        ]
        for table, field, value, predicate in cases:
            with self.subTest(table=table):
                old = self.sql(f"SELECT {field} FROM {table} WHERE {predicate}").strip()
                self.sql(f"UPDATE {table} SET {field}='{value}' WHERE {predicate}")
                before = self.snapshot()
                result = self.seed()
                self.assertNotEqual(result.returncode, 0, result.stdout)
                self.assertEqual(self.snapshot(), before)
                self.sql(f"UPDATE {table} SET {field}='{old}' WHERE {predicate}")


if __name__ == "__main__":
    unittest.main()
