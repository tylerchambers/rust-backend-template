# Contributing and agent handoff

1. Read README, `docs/adr-001.md`, `docs/acceptance.md` and the governing specification snapshots. For scope changes, read the live Notion hub and affected links; do not edit Notion without explicit authorization. Magic-link/current-stable overrides supersede historical passkey/MSRV text.
2. Work on a feature branch. Record ticket IDs, file ownership and migration ownership before parallel work. Only one writer owns shared generated contracts and the combined-tree verification snapshot. Reviewers remain read-only until that snapshot is announced.
3. Keep business policy in domain/application. Domain knows no HTTP, SQLx, Redis or River. HTTP binds transport UUIDs into typed IDs and calls application operations. Infrastructure unwraps IDs into SQL; it owns authoritative locks, RLS and publication safeguards. Bootstrap constructs dependencies explicitly.
4. Write a failing behavioral regression before changing production behavior. Preserve existing assertions. Fakes belong at owned ports; use real PostgreSQL for SQL/transaction/RLS claims, real SMTP peers for transport claims and native Chromium for browser-cookie/confirmation claims.
5. Never rewrite applied migrations, hand-edit SDK/schema outputs, add a fake login route, log credentials or run against personal/production dependencies. No host installs: tooling goes in `.local/tools`; dependencies are disposable exact-ID-owned containers. The preflight prevents implicit rustup installation.
6. Run `python3 scripts/lint.py fmt` for safe fixes and `python3 scripts/lint.py check` for the warning-as-error policy. Every new crate must inherit `[lints] workspace = true`; do not disable lint groups. Document any function-scoped exception with a reason and preserve generated ownership. See `docs/evidence/lint-policy.md`. Run `python3 scripts/verify.py` (or `just check`) on the stable combined tree. This includes generation drift, security/advisory checks and old/current executable migration compatibility. Do not run a second suite against its fixed ports. An earlier green snapshot is not proof of later changes.
7. Review the complete diff, status and retained evidence before committing. Commit/push the authorized feature branch, verify the remote SHA and private repository metadata, and update the existing WIP PR. Do not merge or deploy without authorization.
8. Save a complete `docs/evidence/handoff.md`: reviewed revision/scope, changed behavior, commands and actual exits, every finding with disposition, environment deviations and unresolved gates. Return only the concise outcome to the parent. A WIP PR or finished worker is not release approval.

## Evidence entry

```text
Ticket(s):
Contract/invariant:
Implementation boundary:
RED evidence:
GREEN and integration evidence:
Concurrency/security cases:
Migration/compatibility effect:
Remaining limitations and owner:
Commit/PR:
```

The parent owns independent review and fresh-checkout reproduction. Do not mark those gates complete from the implementation agent's own run. Keep licensing undecided until the owner chooses it.
