# Build with `cargo build --locked --release --workspace` first.
# The pinned toolchain and Cargo.lock own the build inputs.
FROM docker.io/library/debian:trixie-slim@sha256:1e1a22175fbf1b32157ddaf0219e1db4e5401fab2b94ed285add64d18587cd2d
COPY target/release/server target/release/worker target/release/cli /usr/local/bin/
USER 65532:65532
WORKDIR /app
ENV BIND_ADDRESS=0.0.0.0:8080
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/server"]
