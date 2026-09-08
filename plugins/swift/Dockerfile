FROM swift:6.0
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
