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

RUN ln -s /usr/bin/swift-driver /usr/bin/swiftc && \
    ln -s /usr/bin/clang-17 /usr/bin/clang
