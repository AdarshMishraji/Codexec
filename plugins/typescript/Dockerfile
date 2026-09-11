FROM node:22.14.0-alpine AS builder

RUN npm install -g --no-save typescript@5.7.3

RUN mkdir -p /opt/codexec/node_modules && \
    npm install --prefix /opt/codexec --no-save \
        lodash@4.17.21 \
        @types/lodash@4 \
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

# compile_cmd/run_cmd (see plugin.toml) only ever invoke `tsc` and `node`
# directly - npm and corepack (bundled in every official Node image,
# ~20MB combined) are purely an image-build-time tool here. As with
# Dart/Rust/C#/JavaScript elsewhere in this repo, dropping them means a
# genuinely separate final stage on a fresh base, not a later RUN/rm layer
# on the same `FROM node:...-alpine` (which would just hide them behind a
# whiteout while that base image's own layers, still fetched and stored,
# keep the full install). `node` itself statically links V8/most of its
# own dependencies - `ldd` shows only musl libc, libstdc++ and libgcc_s,
# both trivially available via apk on a bare Alpine base matching this
# image's own (confirmed via /etc/os-release). `tsc` is plain JS (no
# native deps beyond node itself) - `npm install -g` drops the package
# under lib/node_modules/typescript and symlinks bin/tsc from
# /usr/local/bin/tsc. `COPY --from=` dereferences symlinks into plain
# files rather than preserving them, which silently breaks tsc's own
# `require('../lib/tsc.js')` (now resolves relative to the wrong
# directory) - confirmed by hitting exactly that MODULE_NOT_FOUND live, so
# the symlink is recreated explicitly here instead of copied.
FROM alpine:3.21
RUN apk add --no-cache libstdc++ libgcc
COPY --from=builder /usr/local/bin/node /usr/local/bin/node
COPY --from=builder /usr/local/lib/node_modules/typescript /usr/local/lib/node_modules/typescript
RUN ln -s /usr/local/lib/node_modules/typescript/bin/tsc /usr/local/bin/tsc
COPY --from=builder /opt/codexec/node_modules /opt/codexec/node_modules
