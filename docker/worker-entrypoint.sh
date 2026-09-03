#!/bin/sh
set -e

mkdir -p /run/containerd /var/lib/containerd /var/lib/codexec/workspaces

# cgroup v2's "no internal process constraint" blocks enabling a
# controller in a cgroup's cgroup.subtree_control while that cgroup itself
# has member processes. Docker puts this container's own init process (and
# everything it forks, including containerd) directly into the top-level
# delegated cgroup, which then blocks delegating e.g. "memory" down to the
# per-submission cgroups containerd creates under /sys/fs/cgroup/codexec/*
# (surfacing as "cannot enter cgroupv2 ... invalid state" from runc).
# Move our own process tree into a sibling leaf cgroup first so the
# top-level cgroup is empty of member tasks and can delegate controllers.
if [ -f /sys/fs/cgroup/cgroup.controllers ]; then
    mkdir -p /sys/fs/cgroup/init
    echo $$ > /sys/fs/cgroup/init/cgroup.procs
    for ctrl in cpu memory pids; do
        echo "+$ctrl" > /sys/fs/cgroup/cgroup.subtree_control 2>/dev/null || true
    done
fi

containerd >/var/log/containerd.log 2>&1 &
CONTAINERD_PID=$!

trap 'kill "$CONTAINERD_PID" 2>/dev/null' TERM INT

SOCK="${CONTAINERD_SOCKET_PATH:-/run/containerd/containerd.sock}"
NS="${CONTAINERD_NAMESPACE:-codexec}"
echo "waiting for containerd to accept requests on ${SOCK}..."
for i in $(seq 1 150); do
    # A round-trip RPC, not just the socket file's existence: the socket
    # can appear slightly before the gRPC server behind it is actually
    # ready to serve, which previously produced a one-off "transport
    # error" race in codexec-worker's first connection attempt.
    if ctr -a "$SOCK" -n "$NS" version >/dev/null 2>&1; then
        echo "containerd is up"
        break
    fi
    if ! kill -0 "$CONTAINERD_PID" 2>/dev/null; then
        echo "containerd exited during startup, see /var/log/containerd.log" >&2
        cat /var/log/containerd.log >&2
        exit 1
    fi
    sleep 0.2
done

exec codexec-worker
