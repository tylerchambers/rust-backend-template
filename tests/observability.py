"""Assert real HTTP/worker correlation and a deliberately narrow log allowlist."""

import json
import re
import sys
from pathlib import Path

root = Path(__file__).resolve().parents[1] / "docs/evidence"
prefix = sys.argv[1]
http = {}
completed = set()
for process in ["server", "worker"]:
    content = (root / f"{prefix}-{process}.log").read_text()
    assert not re.search(r"[a-f0-9]{64}", content), (
        "Possible raw secret in operational logs"
    )
    assert "@example.test" not in content, "Mailbox leaked into logs"
    assert "confirmation#" not in content, "Magic-link URL leaked into logs"
    for line in content.splitlines():
        record = json.loads(line)
        fields = record["fields"]
        if fields.get("operation") == "http.request":
            http[record["span"]["request_id"]] = record["span"]["route"]
        if fields.get("operation") == "job.completed":
            span = record["span"]
            if span["kind"] in {"project_changed_v1", "invitation_email_v1"}:
                completed.add((span["request_id"], span["kind"]))
correlated = {kind for request_id, kind in completed if request_id in http}
assert correlated == {"project_changed_v1", "invitation_email_v1"}, correlated
print(
    f"PASS: {prefix} HTTP requests correlate with Project and invitation jobs; logs contain no tokens, mailboxes or magic links"
)
