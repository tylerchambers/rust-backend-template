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
and click **Confirm sign in**. Return to Project desk to open an organization,
create a project or rename one. Development email stays in the capture service.

Use `localhost` consistently: the frontend and API share the origin and session
cookies. Sign-in requires mailbox access. The [API contract](http://localhost:58039/openapi.json)
also covers features without a frontend, including invitations and member administration.

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
