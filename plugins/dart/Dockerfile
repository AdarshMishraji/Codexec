FROM dart:3.2.6 AS builder

WORKDIR /opt/dart-template
RUN dart create -t console-simple codexec_deps --force && \
    cd codexec_deps && \
    dart pub add collection && \
    dart pub get

    RUN find /usr/lib/dart/bin/snapshots -type f ! -name kernel-service.dart.snapshot -delete && \
    cd /usr/lib/dart/lib/_internal && \
    rm -f ddc_platform.dill dart2js_platform.dill dart2wasm_stringref_platform.dill \
          dart2wasm_platform.dill dart2wasm_js_compatibility_platform.dill \
          dart2js_server_platform.dill ddc_outline.dill dart2wasm_stringref_outline.dill \
          dart2wasm_outline.dill dart2wasm_js_compatibility_outline.dill && \
    rm -rf js_dev_runtime js_runtime wasm js_shared wasm_js_compatibility && \
    cd /usr/lib/dart && \
    rm -rf bin/resources/devtools bin/utils/gen_snapshot bin/utils/gen_snapshot.sym \
           bin/utils/wasm-opt bin/dartaotruntime bin/dart.sym

FROM debian:bookworm-slim

COPY --from=builder /usr/lib/dart /usr/lib/dart
COPY --from=builder /opt/dart-template /opt/dart-template

COPY --from=builder /root/.pub-cache /root/.pub-cache

RUN ln -s /usr/lib/dart/bin/dart /usr/local/bin/dart
