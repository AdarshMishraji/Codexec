FROM swift:6.0 AS builder
WORKDIR /opt/swift-deps
RUN swift package init --type executable --name codexecdeps
RUN cat > Package.swift <<'EOF'
// swift-tools-version:6.0
import PackageDescription

let package = Package(
    name: "codexecdeps",
    dependencies: [
        .package(url: "https://github.com/apple/swift-algorithms.git", exact: "1.2.0"),
        .package(url: "https://github.com/apple/swift-collections.git", exact: "1.1.4"),
        .package(url: "https://github.com/apple/swift-numerics.git", exact: "1.0.2"),
    ],
    targets: [
        .executableTarget(
            name: "codexecdeps",
            dependencies: [
                .product(name: "Algorithms", package: "swift-algorithms"),
                .product(name: "Collections", package: "swift-collections"),
                .product(name: "Numerics", package: "swift-numerics"),
            ]
        )
    ]
)
EOF
RUN swift build -c release

# SwiftPM only emits a standalone lib<Product>.a for products explicitly
# declared `type: .static` - these 3 packages declare their library products
# as "automatic" (the default), so SwiftPM only ever links their compiled
# objects directly into a consuming executable target, never as a
# standalone archive (confirmed: `swift build --product Algorithms` etc.
# just rebuilds the throwaway codexecdeps executable, no libAlgorithms.a
# appears). Since submissions are compiled directly with swiftc (no SwiftPM
# project), we manually archive each target's already-compiled .o files
# (present under .build/release/<Target>.build/) into our own
# lib<Target>.a, then reference those + the already-emitted .swiftmodule
# interface files directly from swiftc at submission compile time.
RUN mkdir -p /opt/swift-libs/lib /opt/swift-libs/modules /opt/swift-libs/include && \
    cp /opt/swift-deps/.build/release/Modules/*.swiftmodule /opt/swift-libs/modules/ && \
    rm -f /opt/swift-libs/modules/codexecdeps.swiftmodule && \
    for t in Algorithms Collections BitCollections DequeModule HashTreeCollections \
             HeapModule InternalCollectionsUtilities OrderedCollections _RopeModule \
             Numerics RealModule ComplexModule _NumericsShims; do \
        ar rcs "/opt/swift-libs/lib/lib${t}.a" /opt/swift-deps/.build/release/${t}.build/*.o; \
    done && \
    cp -r /opt/swift-deps/.build/checkouts/swift-numerics/Sources/_NumericsShims/include \
          /opt/swift-libs/include/_NumericsShims
# RealModule's compiled interface references the underlying C target
# _NumericsShims (a math-shim target, not a pure Swift module) by name, so
# the ClangImporter needs its module.modulemap findable via an -I path at
# submission compile time too - not just the Swift .swiftmodule files.
#
# swift:6.0-jammy-slim (the official "runtime" variant, ~1/10th the size)
# was tried and rejected for the *whole image* - it ships only the stdlib
# .swiftmodule files and shared runtime libs for *running* an
# already-compiled binary, no swiftc at all (confirmed: doesn't exist
# anywhere in that image), and submissions are compiled per-request inside
# this same container, so the compiler has to be present at runtime.
#
# That said, the full swift:6.0 image bundles a lot beyond swiftc/the
# runtime that a plain compile+run submission never touches: SwiftPM's own
# `swift-package` binary (157MB - irrelevant, submissions are compiled
# directly with swiftc, no SwiftPM project), swift-format (64MB),
# swift-build-sdk-interfaces (37MB), swift-help/swift-demangle/
# swift-build-tool/swift-plugin-server (a few more MB), and the *static*
# standard library variant under lib/swift_static (167MB - only used with
# `-static-stdlib`/`-static-executable`, neither of which compile_cmd
# passes, confirmed by inspecting plugin.toml). None of that is reachable
# from `swiftc` itself.
#
# As with Dart/Rust/C#/JS/TS elsewhere in this repo, actually dropping
# that weight needs a genuinely separate final stage on a fresh base - a
# later RUN/rm layer on top of `FROM swift:6.0` would just hide it behind
# a whiteout while the base image's own layers (still fetched and stored)
# keep the full ~4GB. `swift-frontend` (183MB) and `swift-driver` (38MB,
# what `swiftc` itself symlinks to) are both kept in full, along with the
# *entire* lib/swift tree (657MB - the real compiler+runtime shared
# libraries; not trimmed further here, unlike the other plugins' internal
# stdlib pruning, since Swift's own module/library graph is far more
# interconnected and Apple doesn't document a supported "minimal runtime"
# subset the way Dart's per-target platform.dill split or C#'s per-pack
# layout do) and libexec/swift/linux/swift-backtrace(-static) (9.9MB -
# the crash-backtrace helper the runtime execs on a signal like SIGSEGV;
# kept deliberately so a crashing submission's stderr output matches what
# the untrimmed image would have produced).
#
# swift:6.0 and ubuntu:24.04 are the same Ubuntu release (confirmed via
# /etc/os-release). Figuring out the true minimal dependency set here took
# three rounds of actually running `swiftc` end-to-end against the trimmed
# image, not just eyeballing `ldd` on swift-frontend/swift-driver -
# swiftc's own pipeline (`swiftc -v` traces it) shells out to more than
# just those two:
#   - swift-driver additionally needs libsqlite3 (its incremental-build-
#     record database) and the non-wide libncurses.so.6 (ubuntu:24.04
#     only ships libncursesw.so.6 by default) - missing on the first
#     attempt ("error while loading shared libraries: libsqlite3.so.0").
#   - The actual link step (even for pure Swift code with no C
#     dependencies) shells out to `clang` (Swift's own bundled clang-17,
#     not a full separate LLVM install - just ~59MB) to drive linking,
#     `swift-autolink-extract` (reads autolink directives out of the
#     compiled object files), and `ld.gold` (from `binutils`) as the
#     actual linker - missing on the second attempt
#     ("unableToFind(tool: swift-autolink-extract)"). `ld.gold` in turn
#     needs libc's crt1.o/crti.o/crtn.o (from `libc6-dev`) AND gcc's own
#     crtbeginS.o/crtendS.o plus -lgcc/-lgcc_s (from installing `gcc`
#     itself, same as the C/C++/Rust plugins elsewhere in this repo need
#     for exactly this reason) - missing on the third attempt ("cannot
#     open crtbeginS.o", "cannot find -lgcc").
FROM ubuntu:24.04
RUN apt-get update && apt-get install -y --no-install-recommends \
        libsqlite3-0 libncurses6 binutils gcc libc6-dev \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/lib/swift /usr/lib/swift
COPY --from=builder /usr/bin/swift-frontend /usr/bin/swift-frontend
COPY --from=builder /usr/bin/swift-driver /usr/bin/swift-driver
COPY --from=builder /usr/bin/swift-autolink-extract /usr/bin/swift-autolink-extract
COPY --from=builder /usr/bin/clang-17 /usr/bin/clang-17
COPY --from=builder /usr/lib/clang /usr/lib/clang
COPY --from=builder /usr/libexec/swift /usr/libexec/swift
COPY --from=builder /opt/swift-libs /opt/swift-libs

# `COPY --from=` dereferences symlinks into plain files rather than
# preserving them (confirmed live while trimming the TypeScript plugin,
# where this silently broke tsc's own relative require()) - swiftc and
# clang are themselves symlinks (to swift-driver and clang-17
# respectively) upstream, so both are recreated explicitly here instead
# of copied.
RUN ln -s /usr/bin/swift-driver /usr/bin/swiftc && \
    ln -s /usr/bin/clang-17 /usr/bin/clang
