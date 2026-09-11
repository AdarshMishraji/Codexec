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

FROM alpine:3.21
RUN apk add --no-cache libstdc++ libgcc
COPY --from=builder /usr/local/bin/node /usr/local/bin/node
COPY --from=builder /opt/codexec/node_modules /opt/codexec/node_modules
