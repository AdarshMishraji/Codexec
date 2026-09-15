# Runtime image for codexec-api - the HTTP server (submissions + admin
# routes, dashboard, admin portal). Unlike the worker it never invokes
# runc, never touches cgroups and never unpacks images, so this is a plain
# unprivileged binary on a slim base: no runc/skopeo/umoci, no
# `privileged: true`, no root.

# Cross-compile rather than emulate. Pinning the builder to $BUILDPLATFORM
# runs rustc/cargo natively and targets the foreign arch only at codegen +
# link time, via Debian's cross-gcc. Building this workspace inside a
# QEMU-emulated foreign-arch stage instead reliably dies with "cc:
# internal compiler error: Segmentation fault signal terminated program
# collect2" (emulated collect2/lld on a link this size), and is glacially
# slow even when it survives. Only the thin runtime stage below is
# emulated, and it does nothing heavier than apt-get.
FROM --platform=$BUILDPLATFORM rust:1-bookworm AS builder
ARG TARGETARCH
WORKDIR /build

# Resolve TARGETARCH -> rust target triple + toolchain, and stash the
# cross env in a file the build step sources. Written as a file rather
# than ENV because the values depend on TARGETARCH, and a native build
# must export *nothing* - it uses the image's own default cc.
#
# The libc6-dev-*-cross package is deliberate: it's only a Recommends of
# the cross-gcc, so --no-install-recommends drops it, and without the
# target's libc headers every cc-rs build script (ring, here) falls back
# to the build arch's /usr/include and dies on "bits/libc-header-start.h:
# No such file or directory".
RUN set -eux; \
    case "$TARGETARCH" in \
        amd64) triple=x86_64-unknown-linux-gnu;  prefix=x86_64-linux-gnu;  pkg=gcc-x86-64-linux-gnu;  libc=libc6-dev-amd64-cross ;; \
        arm64) triple=aarch64-unknown-linux-gnu; prefix=aarch64-linux-gnu; pkg=gcc-aarch64-linux-gnu; libc=libc6-dev-arm64-cross ;; \
        *) echo "unsupported TARGETARCH: ${TARGETARCH}" >&2; exit 1 ;; \
    esac; \
    rustup target add "$triple"; \
    echo "export CARGO_BUILD_TARGET=${triple}" > /cross-env.sh; \
    if [ "$TARGETARCH" != "$(dpkg --print-architecture)" ]; then \
        apt-get update; \
        apt-get install -y --no-install-recommends "$pkg" "$libc"; \
        rm -rf /var/lib/apt/lists/*; \
        upper="$(echo "$triple" | tr 'a-z-' 'A-Z_')"; \
        under="$(echo "$triple" | tr '-' '_')"; \
        { \
            echo "export CARGO_TARGET_${upper}_LINKER=${prefix}-gcc"; \
            echo "export CC_${under}=${prefix}-gcc"; \
            echo "export AR_${under}=${prefix}-ar"; \
        } >> /cross-env.sh; \
    fi

COPY Cargo.toml Cargo.lock* ./
COPY crates crates
# Two compile-time embeds mean these have to be in the build context even
# though the runtime stage below never reads either off disk:
# sqlx::migrate!("../../migrations") bakes in the migrations
# (RUN_MIGRATIONS_ON_STARTUP replays them from the binary), and
# plugin_templates.rs include_str!'s every plugins/*/plugin.toml to serve
# /admin/plugin-templates.
COPY migrations migrations
COPY plugins plugins

RUN set -eux; \
    . /cross-env.sh; \
    cargo build --release -p codexec-api; \
    cp "target/${CARGO_BUILD_TARGET}/release/codexec-api" /codexec-api

# Builds the React SPA (dashboard/admin/docs) that codexec-api serves via
# tower-http's ServeDir at runtime - see main.rs's fallback_service. Pinned
# to $BUILDPLATFORM like the Rust builder above, but for the opposite
# reason: the output is plain static HTML/JS/CSS with zero architecture
# dependency, so emulating this stage for a foreign TARGETARCH would be
# pure waste, not a correctness workaround.
FROM --platform=$BUILDPLATFORM node:22-bookworm-slim AS frontend-builder
WORKDIR /frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
RUN npm run build

FROM debian:bookworm-slim

# curl is here only so the container has a self-contained healthcheck
# probe (see docker-compose.yml); tini reaps nothing interesting for the
# API server but keeps signal handling (SIGTERM -> graceful exit) correct
# for a PID 1 that isn't written to be an init.
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    tini \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --system --create-home --uid 10001 codexec

COPY --from=builder /codexec-api /usr/local/bin/codexec-api
COPY --from=frontend-builder --chown=codexec:codexec /frontend/dist /home/codexec/static

USER codexec
WORKDIR /home/codexec

# Matches API_BIND_ADDR's default (0.0.0.0:8080); override both together
# if you bind elsewhere.
EXPOSE 8080

ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/codexec-api"]
