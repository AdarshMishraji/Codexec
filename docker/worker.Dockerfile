# Dev/test image only: bundles containerd + runc alongside codexec-worker so
# the real containerd-backed execution engine can be exercised on a dev
# machine (e.g. macOS via colima/Docker Desktop) without a bare-metal Linux
# host. Worker and containerd run as sibling processes in this SAME
# container, sharing one mount/cgroup namespace by design — that's what
# makes the OCI spec's cgroupsPath (set by codexec-exec-engine) and the
# workspace bind-mount resolve identically for both the process that
# creates them (containerd/runc) and the process that reads them back
# (codexec-worker's cgroup stats reader). Splitting them into separate
# containers would require sharing the host cgroup namespace across
# siblings, which is unnecessary complexity for a dev/test setup.

FROM rust:1-bookworm AS builder
WORKDIR /build

RUN apt-get update && apt-get install -y --no-install-recommends \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock* ./
COPY crates crates
COPY migrations migrations

RUN cargo build --release -p codexec-worker -p codexec-plugin-cli

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    containerd \
    runc \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/codexec-worker /usr/local/bin/codexec-worker
COPY --from=builder /build/target/release/codexec-plugin-cli /usr/local/bin/codexec-plugin-cli
COPY docker/worker-entrypoint.sh /usr/local/bin/worker-entrypoint.sh
RUN chmod +x /usr/local/bin/worker-entrypoint.sh

# So `docker compose exec worker codexec-plugin-cli register --manifest
# plugins/<slug>/plugin.toml` works out of the box for local dev/testing.
WORKDIR /app
COPY plugins ./plugins

ENTRYPOINT ["/usr/local/bin/worker-entrypoint.sh"]
