# Rust SaaS backend template

A Rust backend starter for applications where users belong to organizations and
share projects. It includes a JSON API, a background worker and a React workspace.

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

Install [just](https://just.systems/), rustup, the exact Bun version in
[`.bun-version`](.bun-version), and Docker with Compose v2. From the checkout,
`rustup toolchain install` installs the compiler, rustfmt and Clippy specified in
[`rust-toolchain.toml`](rust-toolchain.toml). Start Docker, then run:

```sh
just dev
```

This starts PostgreSQL and Mailpit, runs migrations, builds the frontend and
starts the API and worker. Open [Project desk](http://localhost:58039/) and
enter your email. A new account starts with no organizations.

For the sample organizations, run `just seed` before first signing in as
`owner@example.test`. The seed is optional; startup never overwrites project edits.
Ctrl-C stops the API and worker. `just down` stops PostgreSQL and Mailpit
without deleting the database.

Find the email at <http://localhost:58029>, open the link in the same browser,
and click **Confirm sign in**. Return to Project desk to open an organization.
The Projects tab lets you create or rename projects; the Members tab shows
member IDs and roles. Owners and admins can also see member emails, manage
roles and invitations, and read the organization’s audit events.

To invite someone, revoke an invitation, change a role or remove a member,
review the action and request a proof. Open the confirmation email, check the
action and target, and reveal the proof. Paste it into the original tab to
confirm. Each proof is single-use and expires in five minutes. Recipients sign
in with the invited email address and accept from their invitation link.

Open the session controls next to your signed-in email to list or revoke
sessions, rotate the current credential, or sign out everywhere. Use
`localhost` consistently: the frontend and [API](http://localhost:58039/openapi.json)
share the origin and cookies. Development email stays in the capture service.

## Development

With `just dev` running, use `just frontend` in a second terminal to rebuild the
browser assets on save. Refresh the page to see changes.

Run `just check` for the container-free checks. Run `just integration` for the
database, worker and browser tests with disposable services. `just --list`
shows all recipes.

See [Contributing](CONTRIBUTING.md) for testing and contract changes,
[architecture](docs/adr-001.md) for module boundaries and
[operations](docs/operations.md) for deployment.

## License

[MIT](LICENSE) © 2026 Tyler Chambers.
