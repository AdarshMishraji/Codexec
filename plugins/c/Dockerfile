FROM debian:trixie-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        gcc-14 \
        libc6-dev \
        libasan8 \
        libglib2.0-dev \
        pkg-config \
        uthash-dev \
    && rm -rf /var/lib/apt/lists/*
