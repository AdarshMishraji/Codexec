#!/bin/sh
set -e

mkdir -p /run/codexec/runc /var/lib/codexec/images /var/lib/codexec/workspaces

# cgroup v2's "no internal process constraint" blocks enabling a
# controller in a cgroup's cgroup.subtree_control while that cgroup itself
# has member processes. Docker puts this container's own init process
# directly into the top-level delegated cgroup, which then blocks
# delegating e.g. "memory" down to the per-submission cgroups runc creates
# under /sys/fs/cgroup/codexec/* (surfacing as "cannot enter cgroupv2 ...
# invalid state" from runc, or missing memory.events/pids.max files).
# Move every process currently in the top-level cgroup into a sibling leaf
# cgroup first, so the top level is fully empty of member tasks and can
# delegate controllers - not just our own $$: with tini as PID 1 (see the
# Dockerfile), tini itself is also a top-level member alongside this
# script, and moving only "$$" leaves tini behind, which still trips the
# same constraint. This is unrelated to runc vs containerd - it's a
# property of nesting any cgroup-managing process inside a Docker
# container - see DEPLOYMENT.md for the bare-metal case, which doesn't
# need this at all.
if [ -f /sys/fs/cgroup/cgroup.controllers ]; then
    mkdir -p /sys/fs/cgroup/init
    while read -r pid; do
        [ -n "$pid" ] && echo "$pid" > /sys/fs/cgroup/init/cgroup.procs 2>/dev/null
    done < /sys/fs/cgroup/cgroup.procs
    for ctrl in cpu memory pids; do
        echo "+$ctrl" > /sys/fs/cgroup/cgroup.subtree_control 2>/dev/null || true
    done
fi

exec codexec-worker
