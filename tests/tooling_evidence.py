"""Current-run evidence must include failed and interrupted gates."""

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from evidence import Evidence


class TestEvidence(unittest.TestCase):
    def test_overall_failure_cannot_look_complete(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            evidence = Evidence(root)
            with self.assertRaises(RuntimeError):
                with evidence.session(["cleanup"]):
                    with evidence.gate("cleanup"):
                        pass
                    raise RuntimeError("allocation failed outside command")
            run = json.loads((root / "run.json").read_text())
            self.assertEqual(run["status"], "failed")
            self.assertIn("allocation failed", run["error"])

    def test_timeout_and_failure_replace_prior_results(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "results.json").write_text('[{"gate":"stale"}]')
            evidence = Evidence(root)
            with self.assertRaises(subprocess.TimeoutExpired):
                evidence.run(
                    "timeout",
                    [
                        sys.executable,
                        "-c",
                        "print('started', flush=True); import time; time.sleep(60)",
                    ],
                    timeout=0.1,
                )
            with self.assertRaises(RuntimeError):
                with evidence.gate("drift"):
                    raise RuntimeError("drift")
            results = json.loads((root / "results.json").read_text())
            self.assertEqual([r["gate"] for r in results], ["timeout", "drift"])
            self.assertEqual([r["exit_code"] for r in results], [124, 1])
            self.assertIn("started", (root / "timeout.log").read_text())
            self.assertEqual(results[0]["run_id"], results[1]["run_id"])


if __name__ == "__main__":
    unittest.main()
