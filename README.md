# Rust SaaS backend template

A Rust backend starter for applications where users belong to organizations and
share projects. It provides a JSON API and a background worker; you bring the
frontend and your application's business logic.

The example application supports:

- Passwordless email sign-in and server-side sessions, with session rotation and revocation.
- Organizations, email invitations and owner/admin/member/viewer roles.
- Creating, listing and renaming projects, with version checks to prevent lost updates.
- Tenant isolation through PostgreSQL row-level security, plus audit records for identity and membership changes.
- Background invitation delivery, project indexing and identity cleanup through River.

The stack is Axum, SQLx and PostgreSQL, with optional Redis caching and rate
limiting. [OpenAPI](openapi.json) and a [typed TypeScript client](sdk/src/index.ts)
are included. The Rust workspace separates domain logic, application services,
infrastructure and HTTP handlers.

## Quick start

Install Rust **1.99.1**, Bun **1.4.4**, Python **3.14+**, uv, rootless Podman
(with slirp4netns) and OpenSSL before starting. The scripts do not install Rust
for you. The first run needs network access to download crates and container images.

From the repository root:

```sh
python3 scripts/dev.py
```

This starts local PostgreSQL, Redis and an email capture service, applies migrations,
builds the app, and runs the API and worker. Keep it running. Ctrl-C stops the app
and removes dependencies it started, including their development data.

In another terminal:

```sh
python3 scripts/seed.py
curl --fail http://localhost:58039/health
```

The seed creates two sample organizations and projects owned by
`owner@example.test`. The health check returns HTTP 200 with an empty body.
There is no application dashboard; the API contract is at
<http://localhost:58039/openapi.json>.

### Try signing in

1. Open <http://localhost:58039/v1/auth/confirmation> in your browser.
2. Run this in that page's developer console to request a sign-in email:

   ```js
   await fetch('/v1/auth/request', {
     method: 'POST',
     headers: {
       'content-type': 'application/json',
       'x-csrf-token': document.cookie.split('; ').find(v => v.startsWith('csrf=')).slice(5)
     },
     body: JSON.stringify({email: 'owner@example.test'})
   });
   ```

3. Find the email at <http://localhost:58029>, open its link in the same browser,
   and click **Confirm sign in**. Development email stays in the capture service.
4. Visit <http://localhost:58039/v1/organizations> to see your sample organizations.

Use `localhost` consistently so the origin and session cookies match. Sign-in
requires mailbox access; the template has no separate MFA or account-recovery flow.

## Development

Run the full verification suite with `python3 scripts/verify.py`. It covers Rust
and SDK checks, database integration, browser journeys and containerized execution.
Run only one development instance or verification suite at a time; they share fixed
local ports. Optional `just` shortcuts are listed in the [justfile](justfile).

See [Contributing](CONTRIBUTING.md) for development rules,
[architecture](docs/adr-001.md) for module boundaries,
[operations](docs/operations.md) for configuration and deployment, and
[verification](docs/acceptance.md) for test details.

## License

[MIT](LICENSE) © 2026 Tyler Chambers.
