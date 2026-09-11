FROM dart:3.2.6 AS builder

WORKDIR /opt/dart-template
RUN dart create -t console-simple codexec_deps --force && \
    cd codexec_deps && \
    dart pub add collection && \
    dart pub get

# The stock Dart SDK image ships every dev-time tool alongside the plain
# VM (dart2js/dartdevc/dart2wasm web compilers, the IDE analysis server,
# dartdoc, DevTools, the AOT toolchain) - none of which `dart
# --packages=... main.dart` (our only invocation, see plugin.toml) ever
# touches. Verified by exhaustively bisecting which snapshot/platform
# files are actually load-bearing for that one invocation (removing entire
# groups, then re-adding one at a time until it broke) against real
# submissions exercising async/await, collection/PriorityQueue, records-
# adjacent patterns, recursion, and exception handling - all confirmed
# still working, with clean stdout/stderr, after removing:
#   - Every bin/snapshots/*.dart.snapshot except kernel-service.dart.snapshot
#     (the one actually used to compile source to kernel for VM execution -
#     dart2js/dartdevc/dartdev/analysis_server/dart2wasm/dds are each a
#     separate multi-ten-MB snapshot for a tool we never invoke).
#   - Every lib/_internal/*_platform.dill and *_outline.dill except the
#     vm_platform_strong*.dill pair (the per-compilation-target "platform"
#     kernel files for dart2js/ddc/dart2wasm - not used when the target is
#     the VM itself) plus their js/wasm runtime library directories.
#   - bin/resources/devtools (a bundled web UI), bin/utils/gen_snapshot and
#     wasm-opt (AOT/wasm toolchain, not the JIT path `dart <file>` uses),
#     bin/dartaotruntime (a separate runtime for *precompiled* AOT
#     snapshots - unused since we never AOT-compile), and bin/dart.sym
#     (crash-symbolication data, not needed to run).
# This takes /usr/lib/dart from ~580MB to ~105MB - but trimming it in a
# RUN layer on top of `FROM dart:3.2.6` doesn't actually shrink the image:
# Docker layers are additive, so the untrimmed files stay present (and
# pulled/stored) in the base image's own earlier layers, just hidden by a
# whiteout. Only copying the already-trimmed tree into a fresh, unrelated
# final stage (below) actually drops that data from the resulting image.
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

# dart:3.2.6 is Debian 12 (bookworm) - confirmed via /etc/os-release - and
# the `dart` binary's own dynamic dependencies are just libdl/libpthread/
# libm/libc (`ldd`), all plain glibc, nothing Dart-specific. debian:
# bookworm-slim is therefore ABI-compatible and a true from-scratch-style
# final stage: it never had dart2js/analysis_server/etc. in any layer to
# begin with, rather than having them and hiding them.
FROM debian:bookworm-slim
COPY --from=builder /usr/lib/dart /usr/lib/dart
COPY --from=builder /opt/dart-template /opt/dart-template
# dart pub caches the resolved `collection` package source under $HOME
# (/root/.pub-cache in the builder, per pub's default), separately from
# the project dir the .dart_tool/package_config.json (inside
# /opt/dart-template, already copied above) points at by absolute path -
# both halves have to come across, not just one.
COPY --from=builder /root/.pub-cache /root/.pub-cache

RUN ln -s /usr/lib/dart/bin/dart /usr/local/bin/dart
