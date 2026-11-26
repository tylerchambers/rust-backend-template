# Specification snapshots

Read-only external planning snapshots; source URLs and stable ticket IDs remain in each file. Snapshot ticket statuses are not implementation or test evidence.

- [Planning hub](41105668422d81c7bef8e2041765b11e.md)
- [Architecture and workspace](41105668422d81ab963cd32f2bc1544b.md)
- [PostgreSQL and tenant isolation](41105668422d81d9b6aae3dee6afce05.md)
- [Identity, magic links and sessions](41105668422d8121b3c4f678c0d5a923.md)
- [Authorization and organizations](41105668422d8105b24cd41cbcff5c31.md)
- [River jobs and Redis](41105668422d81cea84ce93fc74fba65.md)
- [API, OpenAPI and example slice](41105668422d81cfa915e6f40f6d96ad.md)
- [Testing, operations and release](41105668422d81a18132dce525bea57a.md)
- [Architecture RFC](41105668422d81c183c3f9c30cc2ffd6.md)
- [Conceptual data-model RFC](41105668422d8173a3c4de9326f65dc0.md)
- [Authentication RFC](41105668422d815ca38ef56e4867359d.md)
- [Jobs and caching RFC](41105668422d8129a1c2c075dea7959c.md)

Use the [current architecture](../adr-001.md) for implemented scope. A membership now has one admin flag; the old role matrix and privileged email proofs are historical. Identity still uses one verified email with no recovery/passkeys or mutable active tenant.
