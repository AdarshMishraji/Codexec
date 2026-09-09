FROM rust:1.88.0-slim-bookworm

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
