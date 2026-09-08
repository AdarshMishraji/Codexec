FROM rust:1.88.0

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
