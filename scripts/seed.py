"""Seed only the exact disposable PostgreSQL container owned by this checkout."""
import json
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parents[1]
state = json.loads((root / ".local/dependencies.json").read_text())
container = state["postgres"]
if len(container) != 64 or any(c not in "0123456789abcdef" for c in container):
    raise SystemExit("Invalid dependency ownership record")
command = ["podman", "exec", "-i", container, "psql", "-U", "postgres", "-d", "template", "-v", "ON_ERROR_STOP=1"]
subprocess.run(command, input=(root / "dev/seed.sql").read_bytes(), check=True, timeout=30)
result = subprocess.check_output(command + ["-Atc", "SELECT count(*) FROM memberships m JOIN users u ON u.id=m.user_id JOIN organizations o ON o.id=m.organization_id JOIN projects p ON p.organization_id=o.id WHERE u.id='00000000-0000-7000-8000-000000000001' AND u.email='owner@example.test' AND m.role='owner' AND o.id IN ('00000000-0000-7000-8000-000000000101','00000000-0000-7000-8000-000000000102')"], text=True, timeout=30)
assert result.strip() == "2", "Seed identity/tenant/project verification failed"
print("Verified two seeded tenant projects; authenticate owner@example.test through the normal captured magic link")
