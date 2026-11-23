# Operations

## Runtime configuration

| Variable | Contract |
|---|---|
| `DATABASE_URL` | Runtime role must be nonowner, nonsuperuser and NOBYPASSRLS. Dedicated PostgreSQL cluster/database. |
| `PUBLIC_ORIGIN` | Canonical origin, no credentials/path/query/fragment. HTTPS in production. |
| `BIND_ADDRESS` | Listener; default `127.0.0.1:8080`. |
| `ENVIRONMENT` | `development` alone allows loopback HTTP/capture. Other values retain production restrictions. |
| `MAIL_MODE` | `smtp` in production; `capture` only for local development. |
| `SMTP_HOST`, `SMTP_USERNAME`, `SMTP_PASSWORD`, `SMTP_FROM` | Production SMTP on port 587 with verified STARTTLS. API and worker reject an invalid sender mailbox at startup. Credentials stay in deployment secrets, never Git/logs. |
| `SMTP_PORT` | Development loopback capture port; default 1025. |
| `REDIS_URL` | Optional; omit to use PostgreSQL-only authority and rate limits. |

Use a secret manager or restricted environment injection. Avoid DSNs on interactive command lines in real deployments. The development credentials in the launcher/Compose are deliberately nonproduction fixture values.

`deploy/compose.yaml` shows migration/API/worker separation. It is **not deployed or production-certified**. Terminate HTTPS at a trusted proxy; keep backend listeners and metrics private. Client source limits use the actual socket address and ignore forwarded headers. A proxy therefore shares one source bucket unless a future explicitly trusted proxy policy is added; never blindly trust `X-Forwarded-For`.

## Migration and rollback

1. Back up PostgreSQL with the operator's established backup tool and verify restoration in an isolated environment.
2. Serialize deployment migrations. Run `cli migrate` using a deployment credential. SQLx owns application checksums/locking; official River owns its independent schema.
3. Start API and worker using the restricted role. Startup rejects owner/superuser/BYPASSRLS credentials. Do not run migrations in replicas.
4. Gate traffic on `/ready`; `/health` is process liveness only.
5. Do not rewrite applied migrations. This template has never deployed and does not qualify a historical old/current executable pair. Before a real deployment, define supported rolling-upgrade/rollback behavior and verify it against the actual revisions and retained data.

There are no destructive down migrations. A contract/drop migration requires a new compatibility plan and backup/restore exercise. Applied SQL files remain immutable. Migration 2 creates the cluster role `template_app`; a second installation sharing a cluster where that role already exists is not supported by this template's current provisioning boundary. Use a dedicated cluster rather than editing checksums or catching unrelated DDL errors. Migration 7 repairs the prior revision's missing River runtime grants.

## Authentication and incident response

- Confirmation pages clear link fragments from browser history. Opening a replacement link in the same tab selects the new link; only the confirmation button submits or reveals it.
- Login links: 256-bit random token, SHA-256 persistence, ten-minute TTL, atomic one-time consumption. GET is inert; sign-in requires an explicit CSRF-protected POST.
- Reauthentication: five-minute action/user/session/tenant/target binding, consumed atomically with the privileged mutation. For an invitation to join, the target must equal the organization ID. Consuming a proof releases its outstanding-proof slot.
- Sessions: seven-day absolute and thirty-minute idle expiry. Rotation retires the old authentication token but preserves a revocation-only lineage handle. Account-wide revocation checks the current credential in the same transaction that retires sessions. Subsequent revocations preserve the original revocation time and do not duplicate the audit event. Disabled users cannot authenticate.
- Rate limits: PostgreSQL normalized-email issuance bucket (five per fifteen minutes), source bucket across auth initiation/confirmation/reauthentication (120 per fifteen minutes), five outstanding fresh proofs per user. Configured Redis adds a fail-closed distributed limiter. Unknown tokens have no trustworthy mailbox identity; source admission bounds those confirmations without exposing an account lookup.
- On suspected mailbox compromise, disable the user through a reviewed deployment/database operation and revoke sessions. No recovery endpoint, email-change flow or alternate credential can bypass mailbox ownership. Session theft permits ordinary operations until expiry/revocation, but not proof-bound membership changes.

Never enable raw SQL bind logging, request-body logging or SMTP error display. Infrastructure errors map to stable sanitized codes. No log field should contain a mailbox, cookie, raw token, proof, DSN or full magic-link URL.

## River

River 0.6.0 owns fetching, concurrency, retries, leader election, stuck-job rescue and schedules. Default queue concurrency is four. Cleanup runs on leader startup and hourly. River's default job timeout is one minute; rescue eligibility adds its one-hour safety interval, and its rescuer polls periodically. Do not mistake that default for immediate failover.

`jobs.rs` integration tests kill a real worker subprocess while its projection write is blocked, advance only that fixture's persisted attempt timestamp, start a replacement worker and require River's rescue error plus attempt two and one projection row. A second test forces delivery failure and advances only the fixture's scheduled retries, requiring five recorded attempts and terminal `discarded`. Test timestamp advancement accelerates library policy; it does not replace production leasing or retry code.

Project indexing is idempotent by project/version. SMTP cannot provide exactly-once external delivery: a connection loss after acceptance can yield duplicate email on retry. Each retry replaces the invitation hash, so only the newest invitation link remains valid. Accepted/revoked/expired invitations are not sent. Discarded jobs stay observable; inspect the job ID/kind/attempt and safe failure category, repair the dependency and use reviewed River operations. Do not replay privileged application mutations by editing queue state.

## Observability

- JSON stderr events contain server-generated request IDs and matched route templates, never raw URLs.
- Application actor context carries that ID into tenant audit rows and durable Project/invitation payloads. Worker tracing spans restore it with job ID/kind/attempt. `tests/system/observability.py` correlates actual native and OCI HTTP/job logs and rejects token/mailbox/link leakage.
- `/metrics/http`: requests, authentication/authorization denials, rate-limited authentication, aggregate request duration.
- `/metrics`: queue depth/lag, retryable/discarded jobs, retained attempt retries, DB pool size/idle slots; when enabled, Redis requests/errors/aggregate latency.
- These are process counters and database gauges, not a historical telemetry store. Scrape both metrics routes privately. Alert on sustained readiness failures, growing due-job lag, discarded jobs, Redis errors and exhausted pool idle capacity. Tracing is implemented; no OTLP collector/exporter is deployed.

API requests have a deadline; production pools have acquire, statement, lock and idle-transaction timeouts. SMTP operations are bounded. SIGINT/SIGTERM drain the API and stop River; process tests verify the active dev group is reaped. The OCI gate runs nonroot with read-only filesystem, dropped capabilities and no-new-privileges.

## CI and supply chain

`.forgejo/workflows/check.yaml` pins checkout and targets a dedicated trusted runner with preinstalled Rust/Bun/Python/Podman. Fixed fixture ports require serialized jobs. Do not execute untrusted fork code on that runner. At implementation time repository Actions was disabled; no hosted-CI execution or runner provisioning is claimed.

`verify.py` runs current RustSec and Bun advisory scans, locked builds, generated-contract drift and digest-pinned fixture/runtime images. Consult the current run evidence for advisory results and database revision; rerun before release. This is not an image-vulnerability scan, SBOM attestation or signing claim. Operators must add their approved image-scanning/signing policy before production distribution.
