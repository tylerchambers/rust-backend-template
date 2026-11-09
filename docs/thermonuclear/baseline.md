# Frozen-main verification baseline

**PASS:** `python3 scripts/verify.py` exited **0**; all **25 top-level gates** exited 0. No implementation fixes were made.

- Worktree: `/home/hermes/rust-backend-template-thermonuclear`
- Frozen revision: `36d86b89d5d3462197ca56c93b5bd9ddc77596b5`
- `git ls-remote origin refs/heads/main` matched that revision before execution; see `baseline/remote-main.log`.
- Started: 2026-11-09T04:23:50.724378+00:00
- Finished: 2026-11-09T04:27:59.032036+00:00
- Elapsed: 248.31 seconds.

## Retained evidence

All paths below are relative to `docs/thermonuclear/`:

- `baseline/command-result.json`: command, actual process exit, timestamps and elapsed time.
- `baseline/verify.log`: complete verifier stdout/stderr, including final dependency cleanup.
- `baseline/evidence-after/results.json`: exact gate commands, exits and durations for this run.
- `baseline/evidence-after/<gate>.log`: gate output; native/OCI server and worker logs, revision compatibility and lifecycle subprocess logs are retained alongside them.
- `baseline/evidence-before/`: complete pre-run evidence snapshot. Historical RED and other pre-existing logs in either snapshot are not new gate failures or proof of new execution; the current gate list is `evidence-after/results.json`.
- `baseline/{containers,ports}-{before,after}.log`: resource inventories.
- `baseline/status-before.log`, `baseline/status-after.log`, `baseline/diff-stat.log`: worktree evidence changes.
- `baseline/{rust,python,bun,uv,podman}.log`: pre-run installed-tool versions.

## Gates

Each gate below exited **0**:

| Area | Gates |
|---|---|
| Disposable fixtures | dependencies, seed, seed-idempotency |
| SDK dependencies and security | sdk-install, bun-audit |
| Static checks and security | lint, lint-negative-controls, architecture |
| Rust and process behavior | rust-tests, dev-lifecycle, release |
| Generated contracts | sdk-generate, sdk-types |
| Native acceptance | native-http, native-metrics, native-sdk, native-browser, native-observability |
| Migration compatibility | revision-compatibility |
| OCI | oci-build, oci-http, oci-metrics, oci-sdk, oci-browser, oci-observability |

The verifier's generated-contract equality assertion also completed. No tracked implementation, OpenAPI or SDK schema changes remained afterward; tracked modifications were confined to normal `docs/evidence/` output.

## Environment and ownership caveats

- Rust 1.99.1 was already installed. Shell Python was 3.14.9; `uv sync --locked` selected already available CPython 3.14.10 for the checkout-local `.venv`, as recorded in `lint.log`. Bun 1.4.4, uv 0.13.7 and Podman 5.7.0 were available.
- No host tools/services were installed or configured. The repository's documented gate provisioned checkout-local tools/dependencies and disposable containers. Existing package/image caches were usable; OCI build explicitly reused cached layers. This is not a cache-cold or hosted-CI run.
- Fixed ports 55439, 51029, 58029, 56379, 58039, 58041 and 58042 were free before execution. This worker alone ran the full suite.
- No `.local/dependencies.json` existed before execution, so the verifier owned its newly created fixtures. It removed that file during successful cleanup.
- Before/after Podman inventories have identical exact container-ID/state mappings: 22 pre-existing containers remain unchanged, with no added container IDs. No unrelated resources were stopped or removed.
- `CONTAINERS_CONF_OVERRIDE` was unset. No process-local networking workaround was needed: the repository dependency launcher already explicitly selects `slirp4netns`.
- The original `/home/hermes/rust-backend-template` checkout was not modified. This run refreshed 20 tracked evidence files in the isolated worktree and added this baseline directory/summary; ordinary ignored build, SDK and tooling artifacts were also produced.
- No implementation fixes, commit, push, deployment, personal mailbox access or hosted CI execution occurred. Passing this bounded suite is not a security audit or release approval. No baseline gate blocker was encountered.
