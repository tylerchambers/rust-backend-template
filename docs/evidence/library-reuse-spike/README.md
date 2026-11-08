# Executed library qualification spike

These are **evidence fixtures, not runtime dependencies or a second application workspace**. The `.txt` suffix prevents accidental inclusion in the application workspace. The snapshots retain the exact compatible dependency lock and executed test source.

Reproduce only against this repository's disposable test PostgreSQL, never production:

```sh
python3 scripts/dependencies.py up
# Use a new directory under your approved scratch directory.
mkdir -p "$SPIKE/src"
cp docs/evidence/library-reuse-spike/Cargo.toml.txt "$SPIKE/Cargo.toml"
cp docs/evidence/library-reuse-spike/Cargo.lock.txt "$SPIKE/Cargo.lock"
cp docs/evidence/library-reuse-spike/lib.rs.txt "$SPIKE/src/lib.rs"
# Set TEST_DATABASE_URL to the disposable deployment/test DSN from scripts/verify.py.
cargo +1.99.1 test --locked --manifest-path "$SPIKE/Cargo.toml"
# Intentional compiler-negative control: must fail with SQLx pool E0308.
cargo +1.99.1 check --locked --manifest-path "$SPIKE/Cargo.toml" --features pool-mismatch
python3 scripts/dependencies.py down
```

`final-test.log`: all three tests passed together using the final source, including duplicate-cookie handling, against real PostgreSQL; temporary `reuse_spike` schema dropped after the store assertions. `initial-test.log` and `csrf-test.log` retain the earlier full and targeted runs. `pool-mismatch.log`: expected E0308, not an application build failure. A duplicate SQLx 0.8 pool exists only inside this experiment to exercise the published store; the application remains on SQLx 0.9/River 0.6.

See [ADR 002](../../adr-002-library-reuse.md) for the decision, published source references and narrow retained exceptions. The spike is not a security certification of either library or application.
