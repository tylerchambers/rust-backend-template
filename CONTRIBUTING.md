# Contributing

Use the [README](README.md#quick-start) to set up the toolchain. Run recipes
from the checkout.

| Command | When to use it |
| --- | --- |
| `just fmt` | Apply formatting before review. |
| `just check` | Check formatting, Clippy, crate boundaries, TypeScript, unit tests and generated-contract drift. Does not need Docker or modify the tracked contracts. |
| `just openapi` | After changing a Rust API definition, regenerate `openapi.json` and the SDK declarations. Review and commit both outputs with the change. |
| `just integration` | Test migrations, database permissions and concurrent transactions against disposable services. Smoke-test the packaged server on `/ready` and run the native API, worker, SDK and browser journeys once. |
| `just audit` | Check Rust advisories, licenses and sources with cargo-deny, and workspace dependencies with Bun audit. The recipe installs its pinned cargo-deny in the checkout. |

Integration uses a separate Compose project and database from `just dev`.
It needs Docker Compose 2.24.4 or later, OpenSSL 1.1.1 or later, curl and
the system libraries for Playwright Chromium. The recipe installs the pinned
headless browser under `.local/playwright`. On a minimal Ubuntu runner, install its system libraries first:

```sh
bun install --frozen-lockfile
bun sdk/node_modules/playwright/cli.js install-deps chromium
```

GitHub Actions runs `just check` and `just integration` on pull requests.
The separate audit workflow runs on dependency changes and weekly.

## Changes to the backend

- Keep business rules in domain and application. HTTP translates requests;
  infrastructure owns SQL, locks and RLS. See the [architecture](docs/adr-001.md).
- In every Rust crate, set `[lints] workspace = true`. Keep lint exceptions
  narrow and explain them with `#[expect(..., reason = "...")]`.
- Add a regression test for changed behavior. Use a real database for
  transaction and RLS claims and a browser for cookie and confirmation flows.
- Never rewrite an applied migration or hand-edit a generated contract.
  Keep credentials, raw tokens and personal databases out of tests and logs.
