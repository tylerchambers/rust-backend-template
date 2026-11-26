# syntax=docker/dockerfile:1
FROM --platform=$BUILDPLATFORM docker.io/oven/bun:1.4.4@sha256:1d22548f177f4ff38e0c15baa3c2c46c6c2060ba2d1bc573bb4720b407548f1d AS frontend
WORKDIR /src
COPY package.json bun.lock bunfig.toml .bun-version ./
COPY sdk/package.json sdk/
COPY apps/frontend/package.json apps/frontend/
RUN --mount=type=cache,target=/root/.bun/install/cache \
    test "$(bun --version)" = "$(cat .bun-version)" && bun install --frozen-lockfile --production
COPY sdk/src/ sdk/src/
COPY apps/frontend/ apps/frontend/
RUN bun run --filter frontend build

# BuildKit selects the target platform's native musl compiler, libc and C assembler.
FROM docker.io/library/rust:1.99.1-alpine@sha256:44819b27be4c97b0e4d7203bd2e02c4820f97700211b6a37fb4c246a2de59383 AS build
ARG TARGETARCH
ENV RUSTUP_TOOLCHAIN=$RUST_VERSION
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY .cargo/ .cargo/
COPY .sqlx/ .sqlx/
COPY migrations/ migrations/
COPY crates/ crates/
COPY apps/server/ apps/server/
COPY apps/worker/ apps/worker/
COPY apps/cli/ apps/cli/
RUN --mount=type=cache,id=rust-backend-registry-${TARGETARCH},target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=rust-backend-target-${TARGETARCH},target=/src/target,sharing=locked <<'EOF'
set -eu
grep -Fxq "channel = \"$RUST_VERSION\"" rust-toolchain.toml
case "$TARGETARCH" in
    amd64) target=x86_64-unknown-linux-musl ;;
    arm64) target=aarch64-unknown-linux-musl ;;
    *) echo "Unsupported architecture: $TARGETARCH" >&2; exit 1 ;;
esac
# Source timestamps can go backwards between checkouts. Reuse compiled third-party
# dependencies, never a workspace artifact from another build.
cargo clean --locked --release --target "$target" --workspace
RUSTFLAGS="-C target-feature=+crt-static" cargo build --locked --release --target "$target" -p server --bin server
cp "target/$target/release/server" /server
readelf --wide --program-headers --dynamic /server > /tmp/server.elf
if grep -E '^[[:space:]]*INTERP[[:space:]]|\(NEEDED\)' /tmp/server.elf; then
    echo "server requires a dynamic loader or shared library" >&2
    exit 1
fi
EOF

FROM gcr.io/distroless/static-debian13:nonroot@sha256:8ce6dd87e9823c5b48dcfb2c8604714ccb351ce59607327bcc4957448da279b5
COPY --from=build /server /usr/local/bin/server
USER 65532:65532
WORKDIR /app
COPY --from=frontend /src/apps/frontend/dist/ /app/apps/frontend/dist/
ENV BIND_ADDRESS=0.0.0.0:8080
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/server"]
