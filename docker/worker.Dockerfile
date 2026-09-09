# Dev/test image: bundles runc + skopeo + umoci alongside codexec-worker so
# the real execution engine can be exercised on a dev machine (e.g. macOS
# via colima/Docker Desktop) without a bare-metal Linux host. No daemon at
# all - runc is a plain subprocess per submission, skopeo/umoci are only
# invoked by codexec-plugin-cli at registration time, not on the
# submission hot path.

FROM rust:1-bookworm AS builder
WORKDIR /build

COPY Cargo.toml Cargo.lock* ./
COPY crates crates
COPY migrations migrations

RUN cargo build --release -p codexec-worker -p codexec-plugin-cli

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    runc \
    skopeo \
    ca-certificates \
    curl \
    tini \
    && rm -rf /var/lib/apt/lists/*

# umoci isn't packaged for Debian bookworm - install the pinned upstream
# static binary directly.
ARG UMOCI_VERSION=0.6.0
RUN ARCH="$(dpkg --print-architecture)" && \
    curl -fsSL -o /usr/local/bin/umoci \
        "https://github.com/opencontainers/umoci/releases/download/v${UMOCI_VERSION}/umoci.linux.${ARCH}" && \
    chmod +x /usr/local/bin/umoci

COPY --from=builder /build/target/release/codexec-worker /usr/local/bin/codexec-worker
COPY --from=builder /build/target/release/codexec-plugin-cli /usr/local/bin/codexec-plugin-cli
COPY docker/worker-entrypoint.sh /usr/local/bin/worker-entrypoint.sh
RUN chmod +x /usr/local/bin/worker-entrypoint.sh

# So `docker compose exec worker codexec-plugin-cli register --manifest
# plugins/<slug>/plugin.toml` works out of the box for local dev/testing.
WORKDIR /app
COPY plugins ./plugins

# `runc create`/`start` fork a container-init process that, once that
# short-lived runc CLI invocation exits, gets reparented to this
# container's PID 1. Without a real init there to reap it, it becomes a
# zombie the moment the container's own process exits - which still holds
# cgroup membership (blocking cleanup) and confuses runc's own state
# reporting (a zombie PID still responds to a liveness check, so runc
# never observes the container as "stopped"). tini is PID 1 instead of
# codexec-worker specifically so it reaps every reparented orphan, not
# just codexec-worker's own direct children.
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/worker-entrypoint.sh"]
