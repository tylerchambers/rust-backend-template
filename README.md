# Rust SaaS backend template

A Rust backend starter for applications where users belong to organizations and
share projects. It includes a JSON API, a background worker and a React workspace.

The example application supports:

- Passwordless email sign-in and server-side sessions, with session rotation and revocation.
- Organizations with email invitations and one admin flag per membership.
- Creating, opening and renaming projects, with version checks to prevent lost updates.
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
Choose **New organization** above the table to create one. After creation,
the table shows your organization and your role.

For the sample organizations, run `just seed` before first signing in as
`owner@example.test`. The seed is optional; startup never overwrites project edits.
Ctrl-C stops the API and worker. `just down` stops PostgreSQL and Mailpit
without deleting the database.

Find the email at <http://localhost:58029> and open its link. Choose
**Confirm sign in** in Project desk. Your workspace opens in the same tab.
Every member can create projects, open them from the Projects table, and rename
them. Admins can also invite people, change admin access, remove members, and
read the organization’s audit events. Each organization must keep one admin.
The breadcrumb trail above the page title links to each parent. From an invitation,
choose **Settings** to return to members or the organization name to return to projects.

Open the cog beside the organization name to see its members. Admins can review
audit events there or choose **Invite member** and confirm the recipient and access.
The recipient signs in with the invited email address and accepts the invitation.

Open your profile menu in the header to log out. Choose **Account settings**
to review active sessions, rotate the current credential, or sign out everywhere. Use
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
