# Opinionated lint policy: decisions and local evidence

## Scope and outcome

Base: `385249dea37465726e0c50bb92f8a7423bf495ca`, existing
`feat/saas-backend`, private PR [#1](https://git.shrimp.rodeo/hermes/rust-backend-template/pulls/1).
The commit containing this report adds an enforced, clean baseline without changing
architecture, migrations, dependency versions, HTTP contracts or generated declarations.
No production deployment, host service installation, project-license selection or merge.

The implementation agent ran the actual complete `python3 scripts/verify.py`:
**23 top-level gates passed**, including **34 passing Rust test results and zero
failures**, native and OCI HTTP/SDK/Chromium journeys, real PostgreSQL/security/RLS/
SMTP/Redis/River tests, lifecycle interruption and old/current executable migration
compatibility. Gate consolidation explains the change from the previous 24 gates;
no security test was removed. Commands, exit codes and timings are in
[`results.json`](results.json), with complete output in each named log.
Parent independent review and fresh-checkout reproduction remain separate gates.

## Rust policy

- Rust **1.99.1**, edition 2024, with pinned rustfmt/Clippy components retained.
  `rustfmt.toml` intentionally contains no options: stable defaults, isolated from
  ancestor-directory configuration. No nightly options or custom formatting style.
- Central `[workspace.lints]`: Rust `unsafe_code = "forbid"`; Clippy `all` and
  `pedantic` at priority -1, plus `unwrap_used`, `expect_used`, `dbg_macro`, `todo`
  and `print_stdout`. Neither the restriction nor nursery group is enabled.
- All **eight** Cargo packages explicitly inherit `[lints] workspace = true`.
  `scripts/lint.py` reads Cargo metadata and verifies every workspace manifest,
  so a newly added crate cannot silently skip the policy. Build/test targets also
  inherit it. The gate runs `cargo clippy --locked --workspace --all-targets
  --all-features -- -D warnings`.
- Public fallible/panicking APIs retain pedantic `missing_errors_doc` and
  `missing_panics_doc`. Forty fallible function contracts now explain errors;
  the static OpenAPI serializer documents its programmer-defect panic. We did not
  enable global `missing_docs` for every DTO field or duplicate trait-implementation
  documentation merely to generate comments.
- Baseline discovery is retained in [`lint-baseline.json`](lint-baseline.json).
  It deliberately ran warnings before enabling the policy; final checks deny warnings.

### Deliberate, bounded exceptions

Production uses function-scoped `#[expect(..., reason = "...")]`, not crate/group
suppression. Unfulfilled expectations become errors in the warning-as-error gate.

| Location / lint | Reason |
|---|---|
| API security middleware / `expect_used` | UUID display is valid ASCII for request-ID headers. Only the two existing UUID conversions remain; static security headers now use `HeaderValue::from_static`. |
| API static contract serializer / `expect_used` | Failure to serialize the derived contract is a schema/programmer defect; panic contract is documented. |
| Bootstrap shutdown / `expect_used` | Preserve the existing fatal failure if the OS signal handler cannot be installed; do not silently continue without shutdown handling. |
| Bootstrap CLI / `print_stdout` | `openapi` deliberately writes the requested document to stdout. Server/worker logging stays structured stderr. Clippy itself exempts build-script Cargo protocol output. |
| API security middleware / `too_many_lines` | Keep ordered ingress/egress policy readable together rather than fragmenting the security boundary to hit a metric. |
| Five existing integration journeys / `too_many_lines` | Shared fixture state and ordered adversarial assertions remain in the same test. |
| HTTP and Redis metrics / `cast_precision_loss` | Prometheus duration sums are approximate floating-point observations, not exact counters used for authority. |
| PostgreSQL row mapper / `needless_pass_by_value` | Function consumes rows directly as an iterator mapping callback. |
| Storage error sanitizer / `needless_pass_by_value` | Consume and discard the sensitive cause at its single logging/sanitization boundary. |

Integration-test crate roots allow only `unwrap_used` and `expect_used` with an
assertion/setup reason. This does not relax production, unsafe, stdout, TODO, debug
macros or other pedantic checks. Existing tests and assertions are preserved;
Clippy's empty-vector assertion rewrites are equivalent, not weakened checks.
No production unwrap remains. The millisecond log conversion now saturates rather
than truncating on an impossible-in-practice duration overflow.

## Python and TypeScript/JavaScript

- Ruff **0.17.4** is exact-pinned in `pyproject.toml` and hash-locked in `uv.lock`.
  `uv sync --locked` installs only into `.venv`; its cache is `.local/uv-cache` and
  Python downloads are disabled. Existing stdlib scripts remain ordinary scripts,
  not a second application framework. Tested using installed Python 3.14.10 and
  uv 0.13.7.
- Ruff formats all Python under `scripts/` and `tests/`. Lints: `E4`, `E7`, `E9`,
  `F`, `I`, `UP`, `B`, `C4` (imports, correctness, modernization and common traps).
  Formatter owns whitespace/line wrapping; no competing formatter or blanket ignores.
  Changes beyond formatting/import order are explicit exception chaining in two
  existing failure paths. Python AST comparison confirmed those limited differences.
- Biome **2.6.6** was already pinned in the SDK manifest/lockfile; this change
  activates it with the stable `recommended` preset, formatting, import organization
  and `--error-on-warnings`. Bun **1.4.4** is checked against `packageManager`.
  The config uses `root: true` and explicit config path to avoid ancestor leakage.
- Biome includes SDK JS/JSX/TS/TSX/CJS/MJS/CTS/MTS sources, including handwritten
  generator scripts and browser/integration tests. `sdk/src/schema.d.ts` and
  `node_modules` are excluded. OpenAPI and SDK declarations are generator-owned;
  byte-for-byte regeneration drift and SDK TypeScript checking still run.
  Embedded HTML/JavaScript inside Rust string literals remains Rust-owned, not
  claimed as a standalone Biome-parsed file.

## Dependency policy and cargo-audit replacement

`cargo-deny` **0.20.2** is installed with `--locked --root .local/tools` and its
actual binary version is checked. `deny.toml` covers all workspace roots, all
features, development dependencies and target-specific reachable dependencies;
there is no target pruning or excluded dependency tree.

- **Advisories:** same upstream RustSec advisory database as the old cargo-audit
  gate, fetched into `.local/advisory-dbs`; no advisory ignores. Vulnerabilities,
  all transitive unmaintained/unsound advisories and yanked releases are enforced.
  `unsound = "all"` is explicit because cargo-deny 0.20.2 otherwise defaults that
  category to workspace-only. `--deny warnings` prevents informational warnings
  from disappearing as a nominally successful gate. Network/database failures fail.
- **Sources:** only crates.io registry sources; unknown registries and all Git
  dependencies are denied. Workspace local paths remain allowed.
- **Bans:** external wildcard requirements and duplicate versions denied.
  `allow-wildcard-paths = true` permits only existing local workspace path edges,
  avoiding meaningless version coupling of unpublished crates.
- **Licenses:** inventory-driven allowlist: MIT, Apache-2.0, BSD-3-Clause, ISC,
  Unicode-3.0, Zlib, 0BSD, BSL-1.0 and CDLA-Permissive-2.0. SPDX alternatives such
  as LGPL-or-MIT can satisfy the policy through MIT; LGPL is not generally allowed.
  Only `riverqueue`, `riverqueue-macros` and `riverqueue-migrate` at **0.6.0** get
  MPL-2.0 exceptions, preserving the explicitly selected official River engine.
  Dependency obligations still apply; this is not a legal clearance/distribution audit.
- **Our license:** `licenses.private.ignore = true` skips only license checking of
  unpublished workspace packages, not their dependency graph. No LICENSE file or
  project license metadata was invented. Public distribution remains an owner decision.

[`dependency-licenses.tsv`](dependency-licenses.tsv) is actual cargo-deny inventory:
246 reachable packages including eight workspace packages and 238 external packages.
It lists available license expressions, not a claim that every alternative is selected.

Version-specific duplicate exceptions (no recursive `skip-tree`):

| Older package skipped for duplicate detection | Upstream reason |
|---|---|
| base64 0.22.1 | SQLx core/PostgreSQL versus lettre/River 0.23 |
| block-buffer 0.10.4, crypto-common 0.1.6, digest 0.10.7, sha2 0.10.9 | SQLx core/macros still use the older crypto generation while PostgreSQL/River/application use the new generation |
| cpufeatures 0.2.17 | Older SQLx SHA-2 versus current SHA-2/rand |
| getrandom 0.2.17 | ring 0.17 versus current rand |
| hashbrown 0.16.1 | SQLx hashlink versus current indexmap |
| syn 2.0.119 | SQLx/tracing/utoipa macros versus current derive ecosystem |
| windows-sys 0.52.0 | ring 0.17 versus Tokio/current platform adapters |

These exceptions affect duplicate detection only, never advisory/source/license
checks. Review them on every lockfile upgrade; unused/mismatched policy emits
warnings that fail the gate. No runtime dependency or Cargo lockfile was changed.

The verifier no longer installs/runs redundant cargo-audit. Its old
`cargo-audit.log`/JSON are historical evidence, not the current gate. The current
Rust advisory result is in [`lint.log`](lint.log), alongside bans/licenses/sources.
`bun audit` remains unchanged for the SDK dependency graph. Ruff is the sole new
Python tool dependency; cargo-deny checks the application Rust graph, not the
independent cargo-install tool dependency graph.

## Commands and rejection evidence

| Executed command | Actual result |
|---|---|
| `python3 scripts/lint.py fmt` | Exit 0; Rust defaults, Ruff and Biome safe fixes/formatting applied; generated SDK untouched |
| `python3 scripts/lint.py check` | Exit 0; all inherited Rust, Ruff, Biome and four cargo-deny checks pass |
| `python3 tests/lint_policy.py` | Exit 0; expected failing probes rejected, positive controls pass |
| `python3 scripts/verify.py` | Exit 0; all 23 top-level gates pass |
| `git diff --check` | Exit 0 |
| `git diff --exit-code -- Cargo.lock openapi.json sdk/src/schema.d.ts sdk/bun.lock` | Exit 0; dependency lock/contract artifacts unchanged |

[`lint-negative-controls.log`](lint-negative-controls.log) retains real diagnostics:
production unwrap, expect, dbg, todo, stdout and missing error documentation each
appear in Clippy's rejected fixture. An attempted local `allow(unsafe_code)` cannot
override workspace `forbid`. A test-only unwrap/expect fixture passes both Clippy
and actual cargo test. Ruff rejects undefined names and unformatted Python; Biome
rejects `any` and unformatted JavaScript. A temporary formatter run leaves copied
generated declarations byte-identical. Altered temporary cargo-deny policies reject
a banned serde, non-allowlisted registry and denied MIT dependencies. Probes live
in disposable checkout-local directories; production sources are never mutated.
Advisories were checked against the real current RustSec database, not a synthetic
vulnerability fixture. This is bounded tooling regression coverage, not a linter
or supply-chain security certification.

Initial setup failures were corrected rather than suppressed: moved misplaced
Python imports, made failure chaining explicit, matched actual Biome/deny diagnostic
text in negative tests, prevented ancestor Biome configuration inheritance, and
adjusted dependency exceptions to the actual inventory. No gate failure remains.

## Review and qualifications

Reviewed tracked/untracked source diff, Python semantic differences, Rust production
and test changes, generated-file ownership, selected lint exceptions and dependency
inventory. [`lint-security-scan.json`](lint-security-scan.json) records the static
scan: JavaScript RegExp `.exec` false positives only, no added production secrets,
shell injection or dynamic execution. Security cases, migration bytes and architecture
boundaries remain intact; no blanket suppression or API restructuring was used.

This report is implementation-agent local evidence, not independent approval.
The parent owns independent final verification. Forgejo repository metadata was
read: private=true, PR open/unmerged, Actions disabled. The workflow calls the same
full verifier and now checks uv availability; no hosted run is claimed and Actions
was not enabled. Publication is a normal non-force push to the existing PR branch;
remote/PR head readback is reported with the exact commit in the final handoff.
