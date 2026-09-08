FROM debian:trixie-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        clang-19 \
        g++-14 \
        libstdc++-14-dev \
        libc6-dev \
        libclang-rt-19-dev \
    && rm -rf /var/lib/apt/lists/*