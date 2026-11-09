# Quality gates

- Rust: stable rustfmt; workspace `unsafe_code = "forbid"`; Clippy `all`, `pedantic`, and selected restrictions. Every crate inherits workspace lints. Warnings fail across all targets/features.
- Exceptions: narrow `#[expect(..., reason = "...")]` declarations only. Integration tests may allow assertion/setup unwraps; production does not inherit that exception.
- Python: pinned Ruff formats and checks `scripts/` and `tests/`.
- SDK: pinned Biome recommended rules and TypeScript checking. Generated declarations are checked by regeneration, not hand-edited.
- Dependencies: cargo-deny enforces RustSec advisories, sources, licenses and version policy; Bun audit checks SDK dependencies. `deny.toml` records narrow version/license exceptions; revisit them on upgrades. Dependency license checks do not choose this project's license.
- Architecture: every workspace crate must have an explicit layer classification; forbidden edges and unclassified crates fail.

Run `python3 scripts/lint.py fmt`, `python3 scripts/lint.py check`, and `python3 tests/lint_policy.py`. The full acceptance command is `python3 scripts/verify.py`. Tools stay checkout-local; the Rust prerequisite guard refuses implicit toolchain installation.

Generated logs and run ledgers under `docs/evidence/` are ignored. Keep concise verification results and review dispositions on the MR, not as committed audit dumps.
