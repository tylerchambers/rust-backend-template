# Independent fresh-checkout acceptance

## Outcome

PASS: commit `e966e69c7be4b3fa9011a287b3a8111e4b80b63d` independently reproduced in `/home/hermes/.hermes/cache/scratch/rust-reuse-acceptance`.
The first unmodified `python3 scripts/verify.py` exited **0** in **245.20 seconds**. This fresh run contains **25 top-level gates**, all exit 0, including dependency creation. Rust reports **39 passed, 0 failed, 0 ignored**, including its helper and compile-fail doctest. No cold-build timeout, separate compile, gate edit, toolchain install, networking override, or test weakening was needed.

## Isolation and prerequisites

Executed from `/home/hermes`:

```sh
git clone --no-hardlinks /home/hermes/rust-backend-template-library-reuse /home/hermes/.hermes/cache/scratch/rust-reuse-acceptance
git -C /home/hermes/.hermes/cache/scratch/rust-reuse-acceptance checkout --detach e966e69c7be4b3fa9011a287b3a8111e4b80b63d
```

Both exited 0. The destination did not exist. Initial git status was clean; `target`, `.local`, `.venv`, and `sdk/node_modules` were all absent. No ignored tools, dependency directories, fixtures or build outputs were copied. Normal existing host Cargo registry/container caches were not purged; this is a fresh checkout, not a cache-empty host. No build/toolchain/network override variables were present. Historical revision `1b266ba502ae8e207a667aa27681913264341ec6` was available as a commit in the full clone; no extra historical fetch was needed.

Read CONTRIBUTING, README, preflight, lint setup implementation, lint-policy, ADR 001, acceptance ledger, full verifier, dependency/seed launcher and revision compatibility test. Loaded tyler-engineering-style and local-forgejo skills. Existing Python 3.14.9, Rust 1.99.1, Bun 1.4.4, uv 0.13.7 and Podman were used. Preflight exited 0. Ruff/Python dependencies, SDK dependencies and cargo-deny were installed by the documented gate into this checkout only. No host binary/service installation was performed.

## Exact gate commands and exits

Working directory is the fresh checkout, except SDK install/audit/generation/typecheck/tests use its `sdk/`. Paths below are literal commands from fresh `docs/evidence/results.json`.

| Gate | Exit | Seconds | Command |
|---|---:|---:|---|
| dependencies | 0 | 23.2 | `python3 scripts/dependencies.py up` |
| seed | 0 | 0.39 | `python3 scripts/seed.py` |
| seed-idempotency | 0 | 0.27 | `python3 scripts/seed.py` |
| sdk-install | 0 | 0.84 | `bun install --frozen-lockfile` |
| lint | 0 | 68.24 | `python3 scripts/lint.py check` |
| lint-negative-controls | 0 | 1.55 | `python3 tests/lint_policy.py` |
| bun-audit | 0 | 0.22 | `bun audit` |
| architecture | 0 | 0.05 | `python3 scripts/architecture.py` |
| rust-tests | 0 | 69.57 | `cargo test --locked --workspace` |
| dev-lifecycle | 0 | 1.64 | `python3 tests/dev_lifecycle.py` |
| release | 0 | 34.64 | `cargo build --locked --release --workspace` |
| sdk-generate | 0 | 0.37 | `bun run generate` |
| sdk-types | 0 | 0.12 | `bun run check` |
| native-http | 0 | 1.02 | `python3 tests/http_acceptance.py` |
| native-metrics | 0 | 0.08 | `/home/hermes/.hermes/cache/scratch/rust-reuse-acceptance/.venv/bin/python tests/metrics.py` |
| native-sdk | 0 | 0.1 | `bun test` |
| native-browser | 0 | 1.74 | `podman run --rm --network host --ipc private --security-opt no-new-privileges -v /home/hermes/.hermes/cache/scratch/rust-reuse-acceptance:/app:ro -w /app/sdk mcr.microsoft.com/playwright:v1.61.1-noble@sha256:5b8f294aff9041b7191c34a4bab3ac270157a28774d4b0660e9743297b697e48 node browser.mjs` |
| native-observability | 0 | 0.02 | `python3 tests/observability.py native` |
| revision-compatibility | 0 | 35.97 | `python3 tests/revision_compatibility.py` |
| oci-build | 0 | 0.43 | `podman build -t localhost/rust-backend-template:test .` |
| oci-http | 0 | 0.82 | `python3 tests/http_acceptance.py` |
| oci-metrics | 0 | 0.05 | `/home/hermes/.hermes/cache/scratch/rust-reuse-acceptance/.venv/bin/python tests/metrics.py` |
| oci-sdk | 0 | 0.1 | `bun test` |
| oci-browser | 0 | 1.3 | `podman run --rm --network host --ipc private --security-opt no-new-privileges -v /home/hermes/.hermes/cache/scratch/rust-reuse-acceptance:/app:ro -w /app/sdk mcr.microsoft.com/playwright:v1.61.1-noble@sha256:5b8f294aff9041b7191c34a4bab3ac270157a28774d4b0660e9743297b697e48 node browser.mjs` |
| oci-observability | 0 | 0.02 | `python3 tests/observability.py oci` |

Native and OCI HTTP, SDK, Chromium, parser-backed metrics and observability passed. Real PostgreSQL/RLS/security/SMTP/Redis/River tests, lifecycle interruption, warning-as-error lint with negative controls, audits, generation drift, release/OCI builds and live old/current revision compatibility passed. These are local disposable-fixture results, not hosted CI or production certification.

## Cleanup and unchanged source

The verifier's normal finalizer removed its owned dependencies; `.local/dependencies.json` is absent. Exact container-ID inventory and volume-name inventory match baseline: **22 preexisting containers and 120 preexisting volumes**, with no additions/removals. No executable from the fresh checkout remains running. All seven fixed ports were confirmed bindable: 55439, 51029, 58029, 56379, 58039, 58041, 58042. No prune or manual resource deletion was used. OCI image/build caches and checkout-local tools/build artifacts remain as evidence; these are not running services.

Two immediate post-run supplemental bind checks exited 1 at SMTP port 51029 because of TCP TIME-WAIT. `ss -ltnp 'sport = :51029'` showed no listener; `ss -tan 'sport = :51029'` showed only TIME-WAIT entries (both commands exit 0). After `sleep 45` (exit 0), the identical supplemental check exited 0 without edits or cleanup intervention. This did not affect the full acceptance gate, which succeeded on its first attempt.

`git diff --exit-code -- . ':!docs/evidence'` and `git diff --check` both exited 0. Only generated evidence changed among tracked files. HEAD remains the requested commit. Original checkout and implementation worktree were not edited; no commit or push was made.

## Retained artifacts and exact wrapper execution ledger

- Fresh suite logs and results: `/home/hermes/.hermes/cache/scratch/rust-reuse-acceptance/docs/evidence/`.
- Supplemental logs: `/home/hermes/.hermes/cache/scratch/rust-reuse-acceptance-logs/` (baseline, verify-1, final-check, final-check-2, final-check-3, commands.jsonl).
- Scratch-only command recorder: `/home/hermes/.hermes/cache/scratch/rust-reuse-acceptance-run.py`.
- Scratch-only cleanup/aggregate checker: `/home/hermes/.hermes/cache/scratch/rust-reuse-acceptance-check.py`.
- This report: `/home/hermes/.hermes/cache/scratch/rust-reuse-fresh-acceptance.md`.

The recorder executes each command unchanged in the fresh checkout, redirects its raw output to the named log, and records exit/duration. Supplemental checker source retains exact inventory, process, port, Git and count assertions. Complete recorded command arrays:

```json
[
  {
    "name": "baseline",
    "command": [
      "python3",
      "-c",
      "import subprocess,pathlib,socket,os; print(\"HEAD\",subprocess.check_output([\"git\",\"rev-parse\",\"HEAD\"],text=True)); print(\"baseline revision\",subprocess.check_output([\"git\",\"cat-file\",\"-t\",\"1b266ba502ae8e207a667aa27681913264341ec6\"],text=True)); print(\"ignored paths\",{p:pathlib.Path(p).exists() for p in [\"target\",\".local\",\".venv\",\"sdk/node_modules\"]}); print(\"build env\",{k:v for k,v in os.environ.items() if k.startswith((\"CARGO_\",\"RUST\",\"CONTAINERS_CONF\"))}); subprocess.run([\"podman\",\"ps\",\"-a\",\"--no-trunc\",\"--format\",\"json\"],check=True); subprocess.run([\"podman\",\"volume\",\"ls\",\"--format\",\"json\"],check=True)\nfor p in [55439,51029,58029,56379,58039,58041,58042]:\n s=socket.socket(); s.bind((\"127.0.0.1\",p)); s.close(); print(\"port free\",p)\nfor c in [[\"python3\",\"--version\"],[\"bun\",\"--version\"],[\"uv\",\"--version\"],[\"rustc\",\"--version\"],[\"python3\",\"scripts/preflight.py\"]]:\n r=subprocess.run(c); print(c,\"exit\",r.returncode); r.check_returncode()"
    ],
    "cwd": "/home/hermes/.hermes/cache/scratch/rust-reuse-acceptance",
    "exit_code": 0,
    "seconds": 0.15,
    "ended": "2026-11-08T04:34:03.652727+00:00"
  },
  {
    "name": "verify-1",
    "command": [
      "python3",
      "scripts/verify.py"
    ],
    "cwd": "/home/hermes/.hermes/cache/scratch/rust-reuse-acceptance",
    "exit_code": 0,
    "seconds": 245.2,
    "ended": "2026-11-08T04:38:14.545846+00:00"
  },
  {
    "name": "final-check",
    "command": [
      "python3",
      "/home/hermes/.hermes/cache/scratch/rust-reuse-acceptance-check.py"
    ],
    "cwd": "/home/hermes/.hermes/cache/scratch/rust-reuse-acceptance",
    "exit_code": 1,
    "seconds": 0.09,
    "ended": "2026-11-08T04:38:39.811170+00:00"
  },
  {
    "name": "final-check-2",
    "command": [
      "python3",
      "/home/hermes/.hermes/cache/scratch/rust-reuse-acceptance-check.py"
    ],
    "cwd": "/home/hermes/.hermes/cache/scratch/rust-reuse-acceptance",
    "exit_code": 1,
    "seconds": 0.09,
    "ended": "2026-11-08T04:38:50.177721+00:00"
  },
  {
    "name": "final-check-3",
    "command": [
      "python3",
      "/home/hermes/.hermes/cache/scratch/rust-reuse-acceptance-check.py"
    ],
    "cwd": "/home/hermes/.hermes/cache/scratch/rust-reuse-acceptance",
    "exit_code": 0,
    "seconds": 0.11,
    "ended": "2026-11-08T04:39:41.009997+00:00"
  }
]
```
