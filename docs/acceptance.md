# Verification

Run `python3 scripts/verify.py` (or `just check`) from a checkout with the README prerequisites. Run one suite at a time: disposable dependencies use fixed loopback ports.

The gate covers:

- Formatting, lint, dependency advisories, architecture rules and negative controls.
- Domain/application contracts and real PostgreSQL transactions, RLS, authentication, proof binding, session pagination and concurrency.
- SMTP, Redis and River integration, including worker recovery.
- Generated OpenAPI/SDK drift and TypeScript checks.
- Native and OCI HTTP, SDK, Chromium, metrics and correlated-log journeys.
- Current migration execution, runtime-role guards and transaction/security contracts.
- Fixture conflicts, cancellation, resource cleanup and truthful run reporting.

## Results

`docs/evidence/` is generated, ignored output. Check `run.json` and matching run IDs in `results.json`: acceptance requires overall `completed`, every expected gate passed and successful cleanup. A partial or failed run is not acceptance. Old log filenames alone do not prove current execution.

Keep the tested revision, command, result and material limitations in the MR. Do not commit logs, review transcripts, handoff documents or copied evidence snapshots. Independent review and fresh-checkout qualification are separate from an implementer's run.

## Limits

Tests use disposable services and captured email, not production or external mail delivery. SMTP retries may duplicate delivery. Historical old-binary compatibility is not a requirement for this undeployed template; qualify real upgrades before deployment. SIGKILL cannot execute cleanup; retained ownership manifests require investigation, never name-based pruning. A checked-in workflow does not prove hosted CI ran. No local gate constitutes production certification.
