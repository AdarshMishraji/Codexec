FROM node:22.14.0-alpine AS builder

RUN mkdir -p /opt/codexec/node_modules && \
    npm install --prefix /opt/codexec --no-save \
        lodash@4.17.21 \
        @datastructures-js/binary-search-tree@5.4.0 \
        @datastructures-js/deque@1.0.8 \
        @datastructures-js/graph@5.3.1 \
        @datastructures-js/heap@4.3.7 \
        @datastructures-js/linked-list@6.1.4 \
        @datastructures-js/priority-queue@6.3.5 \
        @datastructures-js/queue@4.3.0 \
        @datastructures-js/set@4.2.2 \
        @datastructures-js/stack@3.1.6 \
        @datastructures-js/trie@4.2.3

# run_cmd (see plugin.toml) only ever invokes `node` directly - npm and
# corepack (bundled in every official Node image, ~20MB combined) are
# purely an image-build-time tool here, never touched at submission time.
# As with Dart/Rust/C# elsewhere in this repo, dropping them means a
# genuinely separate final stage on a fresh base, not a later RUN/rm layer
# on top of `FROM node:...-alpine` (which would just hide them behind a
# whiteout while that base image's own layers, still fetched and stored,
# keep the full install). `node` itself statically links V8/most of its
# own dependencies - `ldd` shows only musl libc, libstdc++ and libgcc_s,
# both trivially available via apk on a bare Alpine base matching this
# image's own (confirmed via /etc/os-release).
FROM alpine:3.21
RUN apk add --no-cache libstdc++ libgcc
COPY --from=builder /usr/local/bin/node /usr/local/bin/node
COPY --from=builder /opt/codexec/node_modules /opt/codexec/node_modules
