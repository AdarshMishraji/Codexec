# Running codexec on a Linux VM

This covers a bare-metal (or full-VM, non-nested) Linux host: install
containerd directly on the VM and run `codexec-api` (the server) and
`codexec-worker` as native processes. This is simpler than the
`docker-compose.yml` `worker` service, which only exists to nest
containerd+runc inside a container for macOS/Windows dev machines that
don't have Linux containerd available at all. On a real Linux VM you don't
need that nesting trick.

Tested against Ubuntu/Debian; substitute your distro's package manager
where noted.

## 1. Prerequisites

```bash
sudo apt-get update
sudo apt-get install -y \
    build-essential pkg-config protobuf-compiler \
    containerd runc \
    docker.io \
    curl ca-certificates git

# Rust toolchain (skip if already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
```

- `containerd` + `runc` are what actually run submissions — `codexec-worker`
  talks to containerd's gRPC socket directly (see
  `crates/codexec-exec-engine`), never to a Docker daemon.
- `docker.io` here is only used to **build** plugin images (`docker build`);
  it plays no role at submission time. `nerdctl` works equally well if you'd
  rather not run a Docker daemon at all.
- Confirm cgroup v2 is in use (required by the exec engine's resource
  limits): `cat /sys/fs/cgroup/cgroup.controllers` should print something
  like `cpuset cpu io memory pids ...`, not fail with "no such file".
  Modern distro defaults (Ubuntu 22.04+, Debian 12+) already use cgroup v2.

Enable and start containerd:

```bash
sudo systemctl enable --now containerd
sudo systemctl status containerd   # should be active (running)
sudo ctr version                   # sanity check the CLI can reach it
```

You'll also need Postgres and NATS (with JetStream). The repo's
`docker-compose.yml` defines both and is safe to reuse as-is on Linux (only
its `worker` service is the macOS-only nesting hack):

```bash
git clone <your-fork-url> codexec
cd codexec
docker compose up -d postgres nats
```

Or install them natively via your package manager if you'd rather not run
Docker for these — `codexec-api`/`codexec-worker` only need a normal
Postgres connection string and a NATS URL with JetStream enabled
(`nats-server -js`).

## 2. Build

```bash
cd codexec
cargo build --release -p codexec-api -p codexec-worker -p codexec-plugin-cli
```

Binaries land in `target/release/{codexec-api,codexec-worker,codexec-plugin-cli}`.

## 3. Configure

```bash
cp .env.example .env
```

Edit `.env` for your VM. At minimum:

- `DATABASE_URL` — point at your Postgres (`docker-compose.yml`'s default
  is `postgres://codexec:codexec@localhost:5432/codexec`).
- `NATS_URL` — e.g. `nats://127.0.0.1:4222`.
- `ADMIN_API_TOKEN` — required, no default; this Bearer token guards every
  `/admin/*` route (plugin registration, activate/deactivate).
- `CONTAINERD_SOCKET_PATH` — `/run/containerd/containerd.sock` is correct
  for a native `apt install containerd` on the same host.
- `CONTAINERD_NAMESPACE` — `codexec` (containerd creates namespaces
  on demand, nothing to pre-provision).
- `WORKSPACE_ROOT` — a directory the worker process can read/write; each
  submission gets a per-run temp dir bind-mounted into its container here.
  Create it and make sure the user running the worker owns it:
  ```bash
  sudo mkdir -p /var/lib/codexec/workspaces
  sudo chown "$(whoami)" /var/lib/codexec/workspaces
  ```
- `ENGINE_TOTAL_CPU_CORES` / `ENGINE_TOTAL_MEMORY_MB` — raise these from the
  small-dev-box defaults to match your VM's real capacity.

Both binaries load `.env` automatically (via `dotenvy`) if you run them
from the repo root; otherwise export the same variables in your shell or
systemd unit.

## 4. Database migrations

`codexec-api` runs migrations on startup by default
(`RUN_MIGRATIONS_ON_STARTUP=true`), so no separate step is required. To run
them manually instead (e.g. before starting the server in a locked-down
environment), install `sqlx-cli` and run `sqlx migrate run --source
migrations`.

## 5. Run the server

Containerd (and runc) needs root to manage cgroups/namespaces, so the
worker in particular typically runs as root or via a systemd unit with the
right capabilities. The API server itself needs no special privileges.

```bash
./target/release/codexec-api
```

It binds `API_BIND_ADDR` (default `0.0.0.0:8080`). Confirm it's up:

```bash
curl -s http://localhost:8080/languages
```

## 6. Run the worker

```bash
sudo -E ./target/release/codexec-worker
```

(`-E` preserves your shell's env / loaded `.env` when invoking via `sudo`;
adjust to however you're passing config through.)

On a genuine bare-metal/VM host (not nested inside another container),
containerd already has full delegated access to `/sys/fs/cgroup` as root,
so you should **not** need the cgroup `subtree_control` dance that
`docker/worker-entrypoint.sh` does — that workaround exists specifically
because in the dev sidecar setup, containerd's own parent process is
already sitting inside someone else's (Docker's) delegated cgroup. If you
see runc fail with `cannot enter cgroupv2 ... invalid state`, you're likely
running the worker nested inside another container after all; see that
script for the fix.

### Running both as systemd services

```ini
# /etc/systemd/system/codexec-api.service
[Unit]
Description=codexec API server
After=network.target postgresql.service

[Service]
EnvironmentFile=/opt/codexec/.env
ExecStart=/opt/codexec/target/release/codexec-api
Restart=on-failure
User=codexec

[Install]
WantedBy=multi-user.target
```

```ini
# /etc/systemd/system/codexec-worker.service
[Unit]
Description=codexec worker
After=network.target containerd.service
Requires=containerd.service

[Service]
EnvironmentFile=/opt/codexec/.env
ExecStart=/opt/codexec/target/release/codexec-worker
Restart=on-failure
User=root

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now codexec-api codexec-worker
```

## 7. Adding a language plugin

Every plugin is a directory under `plugins/<slug>/` with two files:

- `plugin.toml` — the language's slug/display name/version, its container
  image reference, the compile/run argv and resource limits. See
  `crates/codexec-common/src/registry.rs` for the exact schema, or any
  existing `plugins/*/plugin.toml` for a working example.
- `Dockerfile` (only needed if you're not reusing an existing public
  image) — builds the toolchain image `plugin.toml` points at.

### 7a. Write the plugin

```bash
mkdir plugins/mylang
$EDITOR plugins/mylang/plugin.toml
$EDITOR plugins/mylang/Dockerfile   # skip if reusing a public image as-is
```

Two things worth knowing before you write `compile_cmd`/`run_cmd`:

- Each submission runs in a container with a **read-only root filesystem**
  and only `/sandbox` (containing `in/` and `out/`) writable — see
  `crates/codexec-exec-engine/src/spec.rs`. If your toolchain needs scratch
  space (most native compilers do), redirect `TMPDIR` to `/sandbox/in`.
- The container's `PATH` is hardcoded to the standard system bin dirs
  (`/usr/local/bin`, `/usr/bin`, etc. — see `spec.rs`), **not** whatever
  `PATH` your base image sets via its own `ENV`. If your toolchain lives
  somewhere like `/usr/local/go/bin` only because of the base image's
  `ENV PATH`, symlink the binary into `/usr/local/bin` in your `Dockerfile`
  (see `plugins/go/Dockerfile` or `plugins/rust/Dockerfile` for examples).

### 7b. Build the image

```bash
docker build -t <image.reference from plugin.toml> plugins/mylang
```

### 7c. Get the image into containerd

`codexec-worker` talks to containerd directly and expects the image to
already exist there under the **exact** name in `plugin.toml`'s
`[image] reference` — it never pulls on demand at submission time (see
`crates/codexec-exec-engine/src/image.rs`).

**If `image.reference` is a real, publicly pullable image** (i.e. you
didn't write a custom `Dockerfile` — you're reusing something like
`docker.io/library/python:3.11-slim` as-is), `codexec-plugin-cli register`
handles everything below in one step (pre-pull + DB row), so skip to 7d.

**If you built a custom image locally** (no registry has it — true for
every compiled-language plugin already in this repo), you need to import
it into containerd yourself, since there's nothing to pull:

```bash
docker save <image.reference> | sudo ctr -n codexec images import -
```

⚠️ **Naming gotcha**: `ctr images import` normalizes an unqualified name
like `codexec/mylang:1.0.0` to the fully-qualified `docker.io/codexec/mylang:1.0.0`
on import. If `plugin.toml` says `codexec/mylang:1.0.0` (no `docker.io/`
prefix — the convention every plugin in this repo currently uses), that
string will **not** match what containerd stored, and the worker will fail
every submission with an image-not-found error. Retag it to match exactly
what's in `plugin.toml`:

```bash
sudo ctr -n codexec images tag docker.io/codexec/mylang:1.0.0 codexec/mylang:1.0.0
```

(`images tag` adds an alias to the same content — it doesn't remove the
original name, so this is safe to run even if you're not sure whether it's
needed.)

Verify it landed under the right name:

```bash
sudo ctr -n codexec images ls | grep mylang
```

### 7d. Register the language

If `codexec-plugin-cli` can pull the image itself (public registry case):

```bash
./target/release/codexec-plugin-cli register --manifest plugins/mylang/plugin.toml
```

If you pre-imported it manually (7c's local-image case), running the CLI's
`register` will still try to `ctr image pull` first and fail (there's
nothing to pull) — use the admin HTTP API instead, which registers the DB
row directly with no pull step. It takes JSON, so convert the TOML first:

```bash
python3 -c "
import tomllib, json, sys
with open(sys.argv[1], 'rb') as f:
    print(json.dumps(tomllib.load(f)))
" plugins/mylang/plugin.toml > /tmp/mylang.json

curl -sf -X POST http://localhost:8080/admin/languages \
    -H "Authorization: Bearer $ADMIN_API_TOKEN" \
    -H 'Content-Type: application/json' \
    -d @/tmp/mylang.json
```

(Needs Python 3.11+ for `tomllib`; on older Python, `pip install toml` and
use `toml.load(open(sys.argv[1]))` instead.)

Either path inserts (or upserts, if you're updating an existing plugin)
the language as **active** and notifies already-running workers over NATS
(`codexec.control.plugin_updated`) — **no worker restart needed**, it picks
up new/updated languages live (`crates/codexec-worker/src/registry.rs`).

### 7e. Verify

```bash
./target/release/codexec-plugin-cli list
# or:
curl -s http://localhost:8080/languages | jq
```

Then submit a real test:

```bash
curl -s -X POST http://localhost:8080/submissions \
    -H 'Content-Type: application/json' \
    -d '{"language": "mylang", "source_code": "..."}' | jq

curl -s http://localhost:8080/submissions/<id-from-above> | jq
```

### 7f. Activate / deactivate later

Registering already sets the language active. To toggle it afterward
without touching the image or manifest:

```bash
./target/release/codexec-plugin-cli activate   --slug mylang
./target/release/codexec-plugin-cli deactivate --slug mylang
# or: POST /admin/languages/mylang/activate | deactivate  (same Bearer auth)
```

## 8. Splitting server and worker across hosts, with an autoscaling worker fleet

Everything above can also run as: one (or a small fixed pair behind a load
balancer) `codexec-api` host, and a separate, independently-sized pool of
`codexec-worker` hosts that scales with submission volume. Nothing in
either binary assumes they share a machine — they only share Postgres and
NATS — but a few things that are easy to overlook on a single box become
load-bearing once you split them out.

### 8a. Shared state: Postgres and NATS

Both tiers connect to the same `DATABASE_URL` and `NATS_URL`; run these on
their own host(s) (or a managed Postgres/NATS service), not colocated with
either tier:

- **Network exposure.** The `docker-compose.yml` in this repo binds both
  with no auth beyond Postgres's default password and no TLS — fine on
  `localhost`, not fine once 5432/4222 are reachable from other hosts.
  Restrict both to a security group/firewall rule scoped to just the API
  and worker hosts' private subnet (never expose either publicly), and turn
  on NATS auth (`nats-server -js --auth <token>`, or full user/NKey/TLS
  config) — pass the credential through `NATS_URL` as
  `nats://<user>:<pass>@nats-host:4222`.
- **JetStream persistence.** The submissions stream uses `StorageType::File`
  (see `codexec-api/src/main.rs`'s `ensure_stream`), so put NATS's data
  directory on durable storage — a worker-fleet restart shouldn't lose
  queued submissions.

### 8b. The API host(s)

Nothing containerd-related applies here — `codexec-api` never touches
containerd or runs submissions, so it can live on a small, plain host (or
container) with just the binary, `DATABASE_URL`, `NATS_URL`, and
`ADMIN_API_TOKEN`. It's stateless request/response, so it scales
horizontally the ordinary way (multiple instances behind a load balancer,
no session affinity needed) if request volume ever warrants it — that's a
much less interesting scaling problem than the worker fleet below, since
none of the actual submission execution happens here.

### 8c. The worker fleet

Each worker host still needs everything from §1/§3 that's *local to
execution*: containerd + runc, cgroup v2, a `WORKSPACE_ROOT` directory, and
every plugin image already present in *that host's own* containerd store
(§7c/7d) — containerd's image store is per-host, never shared, so this
doesn't get easier by adding more hosts, it gets repeated on each one. It
does **not** need `ADMIN_API_TOKEN`, `API_BIND_ADDR`, or anything else
API-specific.

Two things make a pool of these hosts behave as one elastic fleet rather
than N independent workers, both already built into `codexec-worker` —
worth understanding before you wire up autoscaling:

- **Work is shared automatically, as long as every instance uses the same
  consumer name.** `codexec-worker` binds to a durable JetStream *pull*
  consumer named `WORKER_CONSUMER_NAME` (default `workers-generic` — see
  `crates/codexec-worker/src/main.rs`'s `get_or_create_consumer` and
  `crates/codexec-worker/src/config.rs`). Multiple processes pulling from
  the *same* durable consumer name automatically compete for messages with
  no duplication — that's what lets you add or remove instances freely.
  Leave `WORKER_CONSUMER_NAME` at its default (or set it, but identically)
  across every instance; giving instances distinct consumer names would
  make each one process a full copy of every submission instead of sharing
  the load.
- **A Postgres row is the real dedup, not JetStream delivery.**
  `handle_message` claims a submission with an atomic
  `UPDATE submissions SET status='processing' ... WHERE status='queued'`
  (`crates/codexec-worker/src/main.rs`) before doing any work, and
  redelivered/duplicate messages that lose that race are ack'd away as
  no-ops. So even a redelivery during a scale-in (an instance terminated
  mid-message, JetStream redelivers to a survivor) is handled correctly —
  you don't need graceful-drain logic beyond a reasonable termination grace
  period (longer than your slowest expected `wall_time_limit_ms`) so
  in-flight containers finish and write back before the process exits.

Because any instance in the pool can be handed any queued submission
regardless of language, **every worker instance must have every active
language's image available locally** — not just newly-launched ones. This
has one real consequence for how you provision plugin images at fleet
scale (§7c's manual `docker save | ctr images import` + retag doesn't
scale to N hosts):

**Push plugin images to a real registry, and use fully-qualified
references in every `plugin.toml`.** Stand up a registry reachable from
every worker host (a self-hosted `registry:2`, or ECR/GCR/Docker
Hub/GHCR/etc.), push each built plugin image there, and set
`image.reference` to the fully-qualified pushed name, e.g.:

```toml
[image]
reference = "registry.internal.example.com/codexec/mylang:1.0.0"
```

This sidesteps §7's local-import naming gotcha entirely (a real registry
pull always stores the image under the exact name you asked for), and it
means `codexec-plugin-cli register`'s built-in pre-pull just works
standalone — no more manual `ctr images import`/`tag` dance for any plugin,
single-host or fleet.

With that in place, provisioning a worker host becomes: install
containerd/runc, then pull every currently-active language's image before
`codexec-worker` starts accepting work. A boot-time script (cloud-init
user-data, or a systemd `ExecStartPre=`) covers both a fresh instance
joining the pool and re-running it manually across the fleet right after
you register a new plugin (§7d) — new instances get it from the registry
automatically, but *already-running* instances only get the new image once
this has run on them too:

```bash
#!/usr/bin/env bash
# provision-worker-images.sh — pull every active language's image into
# this host's containerd store. Run at boot, and again on existing hosts
# whenever a new plugin is registered.
set -euo pipefail
API_URL="${CODEXEC_API_URL:-https://api.internal.example.com}"
NS="${CONTAINERD_NAMESPACE:-codexec}"

curl -sf "$API_URL/admin/languages" -H "Authorization: Bearer $ADMIN_API_TOKEN" \
  | jq -r '.[].image_ref' \
  | sort -u \
  | while read -r ref; do
      echo "pulling $ref..."
      ctr -n "$NS" images pull "$ref"
    done
```

(A fleet-wide command runner — SSM Run Command, Ansible, a small
orchestration tool, whatever you're already using — is what actually
re-runs this on already-live instances after §7d; nothing in
`codexec-worker` pushes new images to running hosts for you.)

### 8d. Autoscaling signal

Worker demand tracks the JetStream consumer's backlog, not request rate —
there's no HTTP traffic to a worker host to measure. Poll the durable
consumer's pending-message count and scale on that:

```bash
nats consumer info SUBMISSIONS workers-generic --json | jq '.num_pending, .num_ack_pending'
```

(`num_pending` = queued and not yet claimed by any instance;
`num_ack_pending` = claimed and currently executing.) Feed this into
whatever autoscaling mechanism your infrastructure already uses — a custom
CloudWatch/Stackdriver metric behind a cloud autoscaling group, a KEDA
`ScaledObject` if the fleet runs on Kubernetes, a small polling script that
calls your cloud API directly, etc.; the two numbers above are the only
codexec-specific input any of them need. Plain CPU/memory-based scaling on
the worker hosts is a reasonable first cut too, since running more
concurrent containers does drive host CPU up, but it lags queue growth
more than measuring the backlog directly.

Whatever drives it, fleet capacity is `WORKER_CONCURRENCY` (concurrent
submissions per instance) × instance count, bounded by each host's real
resources — keep `ENGINE_TOTAL_CPU_CORES`/`ENGINE_TOTAL_MEMORY_MB` (§3)
sized to what the host actually has, since that's what the exec engine
uses to admit or queue new containers locally, independent of whatever
autoscaler is adding more hosts.

### 8e. If the worker fleet runs as containers itself (Kubernetes/Nomad/etc.)

Everything above assumes worker hosts are plain Linux VMs, matching the
containerd-on-bare-metal setup from §1. If you instead run
`codexec-worker` itself inside a container (e.g. Kubernetes pods, one
containerd+runc+codexec-worker per pod, autoscaled via HPA/KEDA on the
same JetStream metric from §8d), you're back in the nested-cgroup
situation the dev `docker-compose.yml` `worker` service exists for —
you'll need that same cgroup `subtree_control` delegation step from
`docker/worker-entrypoint.sh`, a `privileged: true`-equivalent pod security
context, and per-pod-not-per-node image provisioning (§8c's script, run as
each pod's init container instead of at VM boot, since a fresh pod means a
fresh, empty containerd content store every time).
