FROM rust:1.88.0-slim-bookworm AS builder

# The slim variant drops build-essential (unlike the full rust:1.88.0
# image) - rustc still needs a C linker to produce an executable even for
# pure-Rust code with zero C dependencies, since linking itself shells out
# to `cc` by default on Linux.
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

# plugin.toml's compile_cmd invokes `rustc` directly - never `cargo` - so
# cargo itself (its own ~35MB toolchain-managed binary plus the ~19MB
# rustup-shim copy, its registry cache, rustdoc, rust-gdb/lldb, and the
# toolchain's bundled docs/man/zsh-completions) is pure dead weight in the
# image that actually serves submissions. Only three things are load-
# bearing for `rustc` to compile and link a program: the `rustc` binary
# itself, the two shared libraries it and its own driver link against
# (librustc_driver, the LLVM backend), and lib/rustlib (the target's
# precompiled std/core rlibs rustc links every program against - it does
# NOT recompile std from source per invocation). rustc locates its own
# sysroot relative to its own real binary path, so that trio has to keep
# its exact relative layout (bin/rustc next to ../lib/*.so and
# ../lib/rustlib) - confirmed by testing the copied set below directly.
#
# As with Dart elsewhere in this repo, this trimming has to happen via a
# genuinely separate final stage, not a later RUN layer on the same
# `FROM rust:...` image: deleting files in a new layer just hides them
# with a whiteout, it doesn't stop the base image's own earlier layers
# (which already contain the full toolchain) from being stored and
# pulled.
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
