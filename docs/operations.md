# Operations

## Deploy with Docker Compose

Production uses [`compose.yaml`](../compose.yaml) and an existing PostgreSQL database. It has its own Compose project name. `just dev` and `just integration` use `compose.dev.yaml` instead.

1. Build for the deployment host and push the image to your registry. For example, `docker buildx build --platform linux/amd64 -t "$REGISTRY/backend:$VERSION" --push .` (use `linux/arm64` on ARM). Set `BACKEND_IMAGE` to the registry reference **with the digest** returned by the build. The same image runs the server, the worker (`worker`) and the one-shot migrator (`migrate`).
2. Point the public DNS name in `PUBLIC_ORIGIN` at Caddy. Allow inbound TCP 80 and 443 (UDP 443 for HTTP/3) and outbound traffic to the certificate authority. The API and worker need a private route to PostgreSQL, SMTP and any configured Redis instance. Do not open port 8080 or the database port to the internet.
3. Inject the variables in [`.env.example`](../.env.example) through your deployment system, or copy it to a mode-0600 `.env` outside version control. Set both database URLs to the database's certificate hostname with `sslmode=verify-full`. SQLx uses rustls with WebPKI roots; for a private CA, mount its PEM read-only into `migrate`, `api` and `worker` and add `&sslrootcert=/path/in/container/ca.pem` to both URLs. Caddy's HTTPS certificate is unrelated to database TLS.
4. Back up the database, then run `docker compose config --quiet` and `docker compose pull`. On the first install, run `docker compose run --rm migrate` first. Migration 2 creates `template_app` with `NOLOGIN`; provision its login credential or a restricted login with its grants before starting the runtime.
5. Run `docker compose up -d`. Compose waits for the migrator to exit successfully before starting the API and worker. Check `docker compose ps -a`, `docker compose logs api worker` and the `/ready` URL at your public origin. `/health` checks the HTTP process; `/ready` also checks the database. Neither probe needs a shell inside the API.

Caddy redirects HTTP to HTTPS, obtains and renews certificates, and negotiates zstd/gzip. Keep its `/data` and `/config` volumes across deploys; protect `/data` backups as private keys. Do not use `down -v` for routine restarts. Only Caddy publishes ports. The Rust processes run as `65532:65532` with a read-only root, no capabilities and no writable mounts.

Caddy is the internet-facing hop. It does not trust client-supplied forwarded addresses. The API uses the socket peer, not `X-Forwarded-For`, for source-based limits: clients behind Caddy share one source bucket. Do not put a CDN in front and trust arbitrary forwarding headers. The public proxy excludes `/metrics` and `/metrics/http`; scrape `api:8080` over the private network.

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

Keep DSNs out of interactive shell history and logs. `docker compose config` without `--quiet` prints resolved secrets. The development credentials in `justfile` and `compose.dev.yaml` are for local use only.

## Migration and rollback

1. Back up PostgreSQL with the operator's established backup tool and verify restoration in an isolated environment.
2. Serialize deployment migrations. Set `DATABASE_URL` to the deployment credential and run `just migrate` from a checkout, or use the `migrate` Compose service. SQLx owns application checksums and locking; River owns its independent schema. Both migrators ship in the executable.
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
- Application actor context carries the request ID into tenant audit rows and durable project/invitation payloads. Worker tracing spans restore it with the job ID, kind and attempt.
- `/metrics/http`: requests, authentication/authorization denials, rate-limited authentication, aggregate request duration.
- `/metrics`: queue depth/lag, retryable/discarded jobs, retained attempt retries, DB pool size/idle slots; when enabled, Redis requests/errors/aggregate latency.
- These are process counters and database gauges, not a historical telemetry store. Scrape both metrics routes privately. Alert on sustained readiness failures, growing due-job lag, discarded jobs, Redis errors and exhausted pool idle capacity. Tracing is implemented; no OTLP collector/exporter is deployed.

API requests have a deadline; production pools have acquire, statement, lock and idle-transaction timeouts. SMTP operations are bounded. SIGINT/SIGTERM drain the API and stop River. The example deployment runs the API and worker as nonroot with a read-only filesystem, dropped capabilities and no-new-privileges.

## Dependency and image checks

The build checks the release ELF for a program interpreter (`PT_INTERP`) and shared libraries (`DT_NEEDED`) before copying it into the Distroless static nonroot image. SQLx's PostgreSQL driver and Lettre's SMTP transport use rustls/ring; neither needs libpq or OpenSSL at runtime. The only loose application files are the browser build, served at `/` and `/assets` so the same paths also work with the development asset watcher.

For a local build, `docker buildx build --load -t backend:inspect .` followed by `docker image inspect backend:inspect --format '{{.Os}}/{{.Architecture}} {{.Size}} bytes'` reports the local image size. To audit the binary independently, copy `/usr/local/bin/server` out of a stopped container with `docker cp` and inspect it with `readelf -lW` and `readelf -dW` on the host. No inspection tools belong in the runtime image.

`just audit` checks Rust and Bun advisories; `deny.toml` defines Rust license, source and version policies. `just integration` probes the packaged server's readiness. Neither check scans or signs the image.
