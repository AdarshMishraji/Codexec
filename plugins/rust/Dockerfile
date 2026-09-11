FROM rust:1.88.0-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
        gcc libc6-dev \
    && rm -rf /var/lib/apt/lists/*

RUN ln -s /usr/local/cargo/bin/rustc /usr/local/bin/rustc && \
    ln -s /usr/local/cargo/bin/cargo /usr/local/bin/cargo

WORKDIR /opt/rust-deps
RUN cargo init --name codexec_deps --vcs none .
RUN printf '%s\n' \
        'rand = "0.8"' \
        'regex = "1"' \
        'itertools = "0.14"' \
        >> Cargo.toml
RUN cargo build --release --quiet

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
        gcc libc6-dev \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/local/rustup/toolchains/1.88.0-*/bin/rustc /usr/local/rustup/toolchains/1.88.0-linux/bin/rustc
COPY --from=builder /usr/local/rustup/toolchains/1.88.0-*/lib/librustc_driver-*.so /usr/local/rustup/toolchains/1.88.0-linux/lib/
COPY --from=builder /usr/local/rustup/toolchains/1.88.0-*/lib/libLLVM*.so* /usr/local/rustup/toolchains/1.88.0-linux/lib/
COPY --from=builder /usr/local/rustup/toolchains/1.88.0-*/lib/rustlib /usr/local/rustup/toolchains/1.88.0-linux/lib/rustlib
COPY --from=builder /opt/rust-deps/target/release/deps /opt/rust-deps/target/release/deps

RUN ln -s /usr/local/rustup/toolchains/1.88.0-linux/bin/rustc /usr/local/bin/rustc
