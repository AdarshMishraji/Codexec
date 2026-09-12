#!/usr/bin/env bash
# Build and publish the two codexec runtime images to Docker Hub:
#
#   <namespace>/codexec-api     - the HTTP server   (docker/api.Dockerfile)
#   <namespace>/codexec-worker  - the execution node (docker/worker.Dockerfile)
#
# Usage:
#   docker/publish.sh                 # multi-arch build, push to Docker Hub
#   docker/publish.sh --local         # host-arch build, load locally, no push
#   docker/publish.sh --local api     # just one of: api | worker
#
# Env overrides:
#   CODEXEC_IMAGE_NAMESPACE  Docker Hub namespace  (default: codexec)
#   CODEXEC_IMAGE_TAG        tag to publish        (default: latest)
#   CODEXEC_IMAGE_PLATFORMS  buildx platform list  (default: linux/amd64,linux/arm64)
#
# docker-compose.yml reads the same namespace/tag variables, so a stack
# brought up with `docker compose pull && docker compose up -d` lands on
# exactly what this script pushed.

set -euo pipefail

NAMESPACE="${CODEXEC_IMAGE_NAMESPACE:-codexec}"
TAG="${CODEXEC_IMAGE_TAG:-latest}"
PLATFORMS="${CODEXEC_IMAGE_PLATFORMS:-linux/amd64,linux/arm64}"
BUILDER="codexec-builder"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

LOCAL=0
TARGETS=()
for arg in "$@"; do
    case "$arg" in
        --local) LOCAL=1 ;;
        api|worker) TARGETS+=("$arg") ;;
        -h|--help) awk 'NR>1 && /^#/ { sub(/^# ?/, ""); print; next } NR>1 { exit }' "${BASH_SOURCE[0]}"; exit 0 ;;
        *) echo "unknown argument: $arg (expected --local, api or worker)" >&2; exit 2 ;;
    esac
done
[ ${#TARGETS[@]} -eq 0 ] && TARGETS=(api worker)

build() {
    local target="$1"
    local image="${NAMESPACE}/codexec-${target}:${TAG}"
    local dockerfile="docker/${target}.Dockerfile"

    echo "==> ${image}  (${dockerfile})"

    if [ "$LOCAL" -eq 1 ]; then
        # --load can only materialise a single-platform image into the
        # local image store, so host arch only here. This is the mode to
        # use when you just want `docker compose up` to run your working
        # tree rather than whatever is on Docker Hub.
        docker buildx build \
            --file "$dockerfile" \
            --tag "$image" \
            --load \
            "$REPO_ROOT"
    else
        docker buildx build \
            --builder "$BUILDER" \
            --file "$dockerfile" \
            --platform "$PLATFORMS" \
            --tag "$image" \
            --push \
            "$REPO_ROOT"
    fi
}

if [ "$LOCAL" -eq 0 ]; then
    # A multi-platform build needs the docker-container driver; the
    # default "docker" builder can only produce images for the host arch.
    if ! docker buildx inspect "$BUILDER" >/dev/null 2>&1; then
        echo "==> creating buildx builder '${BUILDER}' (docker-container driver)"
        docker buildx create --name "$BUILDER" --driver docker-container >/dev/null
    fi

    if ! grep -q '"https://index.docker.io/v1/"' "${DOCKER_CONFIG:-$HOME/.docker}/config.json" 2>/dev/null; then
        echo "warning: no Docker Hub credentials found - run 'docker login' first if the push fails" >&2
    fi

    # Both Dockerfiles cross-compile (builder stage pinned to
    # $BUILDPLATFORM) rather than building Rust under QEMU, so a
    # foreign-arch build compiles at native speed - only each image's thin
    # apt-get runtime stage is emulated. Narrow with
    # CODEXEC_IMAGE_PLATFORMS=linux/amd64 if you only deploy to x86 VMs.
    echo "==> platforms: ${PLATFORMS}"
fi

cd "$REPO_ROOT"
for target in "${TARGETS[@]}"; do
    build "$target"
done

echo
if [ "$LOCAL" -eq 1 ]; then
    echo "Loaded locally. Run the stack with: docker compose up -d"
else
    echo "Pushed to Docker Hub:"
    for target in "${TARGETS[@]}"; do
        echo "  ${NAMESPACE}/codexec-${target}:${TAG}"
    done
fi
