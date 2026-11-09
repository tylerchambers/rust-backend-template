# Whole-project thermonuclear review

## Final result

One whole-project review round at latest main `36d86b89d5d3462197ca56c93b5bd9ddc77596b5`, followed by remediation and independent verification. All 241 tracked paths were assigned and reviewed exactly once; see inventory.json and review-{core,http,persistence,tooling}.json. Generated outputs were reviewed through source contracts; historical evidence was not counted as new execution.

All 14 initial findings are addressed. Independent follow-up found a page-bound assertion gap (IHTTP-1) and cancellation-during-teardown defect (ITOOL-1); both are addressed. No unresolved blocking findings remain. Historical partial implementation reports and RED logs are preserved as chronology, not current blockers.

## Changes

- CORE-1/2: narrow workflow transaction capabilities sharing one concrete SQL transaction; remove unsupported fake methods; rejected project renames leave state unchanged.
- HTTP-1/2/3: accurate generated errors/HTML/security metadata and typed SDK use; multi-tab CSRF confirmation; safe closed startup diagnostic categories.
- PERSIST-1/2/3: deterministic upfront identity locks and FK-compatible organization locking; stable bounded live-session pagination; readable security-critical SQL. Real PostgreSQL deadlocks reproduced before fixes.
- TOOL-1–6: shared exact-ID/provisional resource ownership; bounded cleanup that defers termination across all owners; truthful run ledger; transactional fixture conflict rejection and narrowly attributed backend cancellation; guarded Rust entrypoints; fail-closed workspace layer classification.

Source OpenAPI and SDK artifacts were regenerated. No applied migrations were rewritten. Session listing now exposes an exclusive `after` cursor with at most 50 rows per page; consumers traverse until an empty page. This is not a cross-request snapshot under concurrent changes. Native confirmation still requires an explicit click and exact Origin/CSRF checks.

## Verification

Baseline: 25/25 gates passed before changes.

Final combined tree: `python3 scripts/verify.py` exited 0; 42/42 gates and 48 Rust test results passed. Run `40d4c59d332643719a1510c240ebf6e3`; canonical ledger in ../evidence/run.json and results.json.

Parent-owned fresh-checkout qualification: cloned the repository into a separate directory and overlaid the exact candidate source, copying no ignored tools, build outputs, node_modules or fixture state. The documented full verifier exited 0 with 42/42 gates and 48 Rust test results. Run `31ac3ebec8b643febd70b9bad712ec80`; fresh-acceptance.log and fresh-evidence/ retain results. Programmatic comparison after execution found zero non-document source differences from the candidate. This is fresh-checkout candidate qualification, not a claim of a cache-cold machine or clone of an already-published commit.

Both runs exercised lint/advisories, generation drift, SQL/RLS/concurrency, native/OCI HTTP/SDK/Chromium/metrics, process and fixture failure paths, transactional seed conflicts, release builds and old/current revision compatibility. Independent review reports: independent-core-persistence.md, independent-http.md, independent-tooling.md, independent-teardown.md. The final targeted reviewer reran 12 teardown signal subcases successfully and closed ITOOL-1.

Added-line/new-file security scan found no credential literals, shell injection, pickle or private keys. Two dynamic-execution pattern matches were JavaScript RegExp.exec calls extracting fixture links, not code execution. git diff --check passed.

## Limits

Forgejo repository metadata confirms private visibility and Actions disabled. Local gates passed; no hosted CI run, deployment or merge is claimed. Container allocation uncertainty is tested with explicit fake-executable fault models; real native/OCI lifecycle and browser journeys pass separately. SIGKILL cannot run cleanup; durable manifests preserve recovery obligations. SQLx CLI preparation was unavailable, but checked query strings/metadata remained unchanged and online/offline compilation passed. Dedicated cancellation-during-new-identity-lock-wait regression was not added; existing transaction ownership is preserved and concurrent lock/expiry/login tests pass.

The original dirty checkout was left untouched. This branch is for owner review, not deployment authorization.
