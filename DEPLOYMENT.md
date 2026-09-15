# Deploying codexec

Four independent paths, pick whichever matches your situation — they're not
mutually exclusive (e.g. it's normal to run Postgres/NATS from Docker while
`codexec-api`/`codexec-worker` run natively):

- **[Running natively on Linux or a Linux-based VM](#running-natively-on-linux-or-a-linux-based-vm)**
  — install `runc`/`skopeo`/`umoci` directly on the host and run both
  binaries as plain (or systemd-managed) processes. The most direct path
  on a real Linux box, and what production should look like.
- **[Running in a Dockerized environment](#running-in-a-dockerized-environment)**
  — `docker compose up`, no Rust toolchain or Linux host required at all
  (useful for macOS/Windows dev machines, since `runc` needs a real Linux
  kernel and this nests it inside a privileged container instead).
- **[Deploying the worker and API server separately](#deploying-the-worker-and-api-server-separately)**
  — splitting `codexec-api` and an autoscaling `codexec-worker` fleet
  across independent hosts, including running Postgres/NATS as their own
  standalone tier.
- **[Downloading prebuilt binaries from GitHub Releases](#downloading-prebuilt-binaries-from-github-releases)**
  — `codexec-api`, `codexec-worker`, and `codexec-plugin-cli` are each
  published as standalone Linux binaries on every version tag, as an
  alternative to `git clone` + `cargo build` for any of the three. Mix and
  match per binary and per host — nothing requires all three to come from
  the same source.

Every binary can be obtained either way — a `git clone` + `cargo build`
(covered inline wherever each binary is introduced below) or a download
from GitHub Releases (covered once, for all three, in the last section) —
independently of which of the four deployment shapes above you're using.
A common real combination: `codexec-api` and `codexec-worker` downloaded
as release binaries on plain VMs, with `codexec-plugin-cli` also
downloaded onto a separate operator laptop that never runs either server
process at all.

---

## Running natively on Linux or a Linux-based VM

This covers a bare-metal (or full-VM, non-nested) Linux host: install
`runc` directly on the VM and run `codexec-api` (the server) and
`codexec-worker` as native processes. This is simpler than
[the Dockerized worker](#running-in-a-dockerized-environment), which only
exists to nest runc inside a container for macOS/Windows dev machines that
don't have a Linux host available at all. On a real Linux VM you don't
need that nesting trick.

Tested against Ubuntu/Debian; substitute your distro's package manager
where noted.

### Prerequisites

```bash
sudo apt-get update
sudo apt-get install -y \
    build-essential pkg-config \
    runc skopeo \
    docker.io \
    curl ca-certificates git

# umoci isn't packaged for Debian/Ubuntu - install the pinned upstream
# static binary directly (see docker/worker.Dockerfile for the same step).
UMOCI_VERSION=0.6.0
ARCH="$(dpkg --print-architecture)"
sudo curl -fsSL -o /usr/local/bin/umoci \
    "https://github.com/opencontainers/umoci/releases/download/v${UMOCI_VERSION}/umoci.linux.${ARCH}"
sudo chmod +x /usr/local/bin/umoci

# Rust toolchain (skip entirely if you're using prebuilt release binaries
# for all three - see "Downloading prebuilt binaries from GitHub Releases"
# below - and only need runc/skopeo/umoci from this block)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
```

- `runc` is what actually runs submissions — `codexec-worker` invokes it
  directly as a subprocess per submission (see
  `crates/codexec-exec-engine`), no daemon, no gRPC, never a Docker daemon
  on the hot path.
- `skopeo` + `umoci` are what `codexec-plugin-cli register` uses to
  pull-and-unpack a plugin's image into a plain rootfs directory ahead of
  submission time — also no daemon involved.
- `docker.io` here is only used to **build** plugin images (`docker build`);
  it plays no role at submission time, nor does it need to be present on
  worker hosts at all once images are pushed to a registry (see
  [The worker fleet](#the-worker-fleet)). `nerdctl` or `buildah` work
  equally well if you'd rather not run a Docker daemon even for building.
- Confirm cgroup v2 is in use (required by the exec engine's resource
  limits): `cat /sys/fs/cgroup/cgroup.controllers` should print something
  like `cpuset cpu io memory pids ...`, not fail with "no such file".
  Modern distro defaults (Ubuntu 22.04+, Debian 12+) already use cgroup v2.

Sanity-check runc directly:

```bash
runc --version
```

No service to enable/start here — unlike containerd, `runc` has no
daemon; `codexec-worker` invokes it fresh per submission.

You'll also need Postgres and NATS (with JetStream). The repo's
`docker-compose.yml` defines both and is safe to reuse as-is on Linux —
name the two services explicitly and you get just the infrastructure,
leaving `api`/`worker` to run natively per below (its `worker` service is
the macOS-only nesting hack; see
[Running in a Dockerized environment](#running-in-a-dockerized-environment)):

```bash
git clone <your-fork-url> codexec
cd codexec
docker compose up -d postgres nats
```

Or install them natively via your package manager if you'd rather not run
Docker for these — `codexec-api`/`codexec-worker` only need a normal
Postgres connection string and a NATS URL with JetStream enabled
(`nats-server -js`). See
[Deploying Postgres and NATS separately](#deploying-postgres-and-nats-separately)
for the production-hardening version of this (auth, TLS, network
exposure) once you're past a single dev box.

### Build

```bash
cd codexec
cargo build --release -p codexec-api -p codexec-worker -p codexec-plugin-cli
```

Binaries land in `target/release/{codexec-api,codexec-worker,codexec-plugin-cli}`.

`codexec-api` also needs the React frontend (the dashboard/admin portal/API
docs) built separately — it's served from disk at runtime
(`STATIC_ASSETS_DIR`, see [Configure](#configure)), not baked into the
binary:

```bash
cd frontend && npm ci && npm run build && cd ..
```

Needs Node 20+ (matches `docker/api.Dockerfile`'s `node:22-bookworm-slim`
frontend-builder stage). Output lands in `frontend/dist`.

Prefer not to build at all? Every command below that invokes
`./target/release/<binary>` works identically against a binary obtained
from [Downloading prebuilt binaries from GitHub Releases](#downloading-prebuilt-binaries-from-github-releases)
instead — install it to `/usr/local/bin` (or anywhere on `PATH`) and use
that path in place of `./target/release/<binary>` everywhere below. This
is a per-binary choice, not all-or-nothing: it's entirely normal to build
`codexec-worker` from source on a worker host while running a downloaded
`codexec-api` binary on the server host, or vice versa.

### Configure

```bash
cp .env.example .env
```

Edit `.env` for your VM. At minimum:

- `DATABASE_URL` — point at your Postgres (`docker-compose.yml`'s default
  is `postgres://codexec:codexec@localhost:5432/codexec`).
- `NATS_URL` — e.g. `nats://127.0.0.1:4222`.
- `ADMIN_API_TOKEN` — required, no default; this Bearer token guards every
  `/admin/*` route (plugin registration, activate/deactivate).
- `IMAGE_CACHE_ROOT` — where `codexec-plugin-cli register` pulls+unpacks
  each plugin's image to (via skopeo+umoci) and where `codexec-worker`
  reads it back from — both must point at the same directory. Default
  `/var/lib/codexec/images` is fine; create it and make sure both the user
  running `codexec-plugin-cli` and the user running `codexec-worker` (root,
  typically — see below) can read/write it.
- `RUNC_ROOT` — `runc`'s own `--root` state directory, tracking
  created/running containers. Default `/run/codexec/runc` is fine; give it
  its own path (rather than runc's default `/run/runc`) so it can't collide
  with any other runc usage on the host (e.g. Docker's own, if `docker.io`
  is also installed per [Prerequisites](#prerequisites)).
- `WORKSPACE_ROOT` — a directory the worker process can read/write; each
  submission gets a per-run temp dir bind-mounted into its container here.
  Create it and make sure the user running the worker owns it:
  ```bash
  sudo mkdir -p /var/lib/codexec/workspaces /var/lib/codexec/images /run/codexec/runc
  sudo chown "$(whoami)" /var/lib/codexec/workspaces /var/lib/codexec/images
  ```
- `ENGINE_TOTAL_CPU_CORES` / `ENGINE_TOTAL_MEMORY_MB` — raise these from the
  small-dev-box defaults to match your VM's real capacity.
- `STATIC_ASSETS_DIR` — where `codexec-api` serves the built frontend from.
  Docker's default (`static`, relative to the image's `WORKDIR`) needs no
  override; running natively from the repo root, set this to
  `frontend/dist` (matching the build step above). Getting this wrong
  doesn't crash the server — the JSON API keeps working — but every UI
  route (`/`, `/admin`, `/docs`) 404s until it points at the right place.

Both binaries load `.env` automatically (via `dotenvy`) if you run them
from the repo root; otherwise export the same variables in your shell or
systemd unit.

### Database migrations

`codexec-api` runs migrations on startup by default
(`RUN_MIGRATIONS_ON_STARTUP=true`), so no separate step is required. To run
them manually instead (e.g. before starting the server in a locked-down
environment), install `sqlx-cli` and run `sqlx migrate run --source
migrations`.

### Run the server

`runc` needs root to manage cgroups/namespaces, so the worker in
particular typically runs as root or via a systemd unit with the right
capabilities. The API server itself needs no special privileges.

```bash
./target/release/codexec-api
```

It binds `API_BIND_ADDR` (default `0.0.0.0:8080`). Confirm it's up:

```bash
curl -s http://localhost:8080/languages
```

### Run the worker

```bash
sudo -E ./target/release/codexec-worker
```

(`-E` preserves your shell's env / loaded `.env` when invoking via `sudo`;
adjust to however you're passing config through.)

On a genuine bare-metal/VM host (not nested inside another container),
`codexec-worker` (running as root) already has full delegated access to
`/sys/fs/cgroup`, so you should **not** need the cgroup `subtree_control`
dance that `docker/worker-entrypoint.sh` does — that workaround exists
specifically because in the dev sidecar setup, the worker's own process is
already sitting inside someone else's (Docker's) delegated cgroup. If you
see runc fail with `cannot enter cgroupv2 ... invalid state`, you're likely
running the worker nested inside another container after all; see that
script for the fix.

Also bare-metal-only: a normal Linux init (systemd as PID 1) already reaps
every orphaned process on the host, so `codexec-worker` doesn't need to be
PID 1 itself the way the dev Docker image needs `tini` in front of it (see
`docker/worker.Dockerfile`) — `runc create`/`start` fork a container-init
helper that gets reparented once those short-lived `runc` invocations
exit, and something has to reap it or it lingers as a zombie holding its
cgroup open. Only relevant if you end up running `codexec-worker` itself
inside a container on this host too (see
[Running the worker fleet inside containers](#running-the-worker-fleet-inside-containers)).

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
After=network.target

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

### Adding a language plugin

Every plugin is a directory under `plugins/<slug>/` with two files:

- `plugin.toml` — the language's slug/display name/version, its container
  image reference, the compile/run argv and resource limits. See
  `crates/codexec-common/src/registry.rs` for the exact schema, or any
  existing `plugins/*/plugin.toml` for a working example.
- `Dockerfile` (only needed if you're not reusing an existing public
  image) — builds the toolchain image `plugin.toml` points at.

#### Write the plugin

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

#### Build the plugin image

```bash
docker build -t <image.reference from plugin.toml> plugins/mylang
```

#### Register the language

`codexec-worker` expects the image already pulled-and-unpacked into
`IMAGE_CACHE_ROOT` under the exact name in `plugin.toml`'s
`[image] reference` — it never pulls on demand at submission time (see
`crates/codexec-exec-engine/src/image.rs`). `codexec-plugin-cli register`
does the pull (via `skopeo`) and unpack (via `umoci`) for you as part of
registering, so there's no separate "get the image into place" step, and
no naming gotcha to worry about — `skopeo`/`umoci` always store things
under the exact reference you give them.

**If `image.reference` is a real, publicly pullable image** (i.e. you
didn't write a custom `Dockerfile` — you're reusing something like
`docker.io/library/python:3.11-slim` as-is):

```bash
./target/release/codexec-plugin-cli register --manifest plugins/mylang/plugin.toml
```

**If you built a custom image locally** (no registry has it — true for
every compiled-language plugin already in this repo, unless you've pushed
it somewhere per [The worker fleet](#the-worker-fleet)), pull straight from
your local Docker daemon's image store instead, with `--source
docker-daemon`:

```bash
./target/release/codexec-plugin-cli register \
    --manifest plugins/mylang/plugin.toml --source docker-daemon
```

Either way, registering is idempotent: an image already present in
`IMAGE_CACHE_ROOT` is left as-is (registering again just updates the DB
row — limits, commands, etc. — without re-pulling). Rebuilt the image
under the same tag and need the new bytes picked up? Add `--force`:

```bash
./target/release/codexec-plugin-cli register \
    --manifest plugins/mylang/plugin.toml --source docker-daemon --force
```

Registering inserts (or upserts, if you're updating an existing plugin)
the language as **active** and notifies already-running workers over NATS
(`codexec.control.plugin_updated`) — **no worker restart needed**, it picks
up new/updated languages live (`crates/codexec-worker/src/registry.rs`).

If you'd rather register without a pull at all (the row already exists,
you only changed limits, and skipping even the presence-check matters to
you), use the admin HTTP API directly instead, which writes the DB row
with no pull step:

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

#### Verify the plugin

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

#### Activating and deactivating a plugin

Registering already sets the language active. To toggle it afterward
without touching the image or manifest:

```bash
./target/release/codexec-plugin-cli activate   --slug mylang
./target/release/codexec-plugin-cli deactivate --slug mylang
# or: POST /admin/languages/mylang/activate | deactivate  (same Bearer auth)
```

---

## Running in a Dockerized environment

Two images are built from this repo, one per binary:

| Image | Dockerfile | What it is |
| --- | --- | --- |
| `codexec/codexec-api` | `docker/api.Dockerfile` | The HTTP server: submissions API, `/admin/*`, dashboard, admin portal. Unprivileged, non-root, no runc — a plain stateless web process. |
| `codexec/codexec-worker` | `docker/worker.Dockerfile` | `codexec-worker` bundled with `runc` + `skopeo` + `umoci`, so the real execution engine runs without a bare-metal Linux host. Needs `privileged: true` (see the caveat below). |

`docker-compose.yml` wires up Postgres, NATS and both of these, so the
entire stack comes up without a Rust toolchain on the host:

```bash
git clone <your-fork-url> codexec
cd codexec
cp .env.example .env              # at minimum set ADMIN_API_TOKEN
docker compose up -d --build      # or: docker compose pull && docker compose up -d
curl -s http://localhost:8080/languages
```

`--build` compiles both images from the working tree; `docker compose
pull` is the shortcut once the tags exist on Docker Hub — which they only
do after someone runs `docker/publish.sh` (below) against a namespace they
own. The `codexec` default is a placeholder: point
`CODEXEC_IMAGE_NAMESPACE` at your own Docker Hub account or org if that
one isn't yours.

The API is on `localhost:8080` (dashboard at `/`, admin portal at
`/admin`). It runs the migrations on startup, so a fresh Postgres volume
needs no extra step. Register plugins the usual way (see
[Adding a language plugin](#adding-a-language-plugin)) — from inside the
worker container, since that's where the image cache lives:

```bash
docker compose exec worker codexec-plugin-cli register --manifest plugins/python3/plugin.toml
```

**Caveat on the worker image:** it exists to nest runc inside a container
for dev machines, and `privileged: true` hands the container effective
root on the host's kernel. That's an acceptable trade on a laptop and a
poor one in production — on a real Linux VM run `codexec-worker` natively
(see [Running natively on Linux or a Linux-based VM](#running-natively-on-linux-or-a-linux-based-vm)),
which is both simpler and safer. The `api` image carries no such caveat
and is fine to deploy as-is.

### Publishing the images

`docker/publish.sh` builds and pushes both. It's the only step that needs
Docker Hub credentials:

```bash
docker login
docker/publish.sh                      # multi-arch (amd64 + arm64), push both
docker/publish.sh --local              # host-arch only, load locally, no push
docker/publish.sh --local worker       # just one image
```

Overridable via the environment — `docker-compose.yml` reads the same two
variables, so a stack you bring up lands on exactly what you pushed:

- `CODEXEC_IMAGE_NAMESPACE` — Docker Hub namespace (default `codexec`);
  set it to your own account to publish a fork.
- `CODEXEC_IMAGE_TAG` — tag to build/run (default `latest`). Prefer an
  immutable tag (a version or commit SHA) for anything you deploy, and
  push `latest` alongside it only as a convenience pointer.
- `CODEXEC_IMAGE_PLATFORMS` — default `linux/amd64,linux/arm64`. Narrow it
  to `linux/amd64` if you only ever deploy to x86 VMs.

Both Dockerfiles **cross-compile** for the foreign arch (builder stage
pinned to `--platform=$BUILDPLATFORM`, linking through Debian's cross-gcc)
rather than building Rust inside a QEMU-emulated stage. That isn't just a
speed choice: an emulated `cargo build` of this workspace reliably dies
with `cc: internal compiler error: Segmentation fault signal terminated
program collect2` — QEMU cannot survive a link that size. Only each
image's thin runtime stage (an `apt-get`, nothing more) is emulated. If
you add a builder step that shells out to a foreign-arch binary, that
trade reverses and you'll be back on the emulator.

To run your working tree instead of a published image, `docker compose
build` (or `docker/publish.sh --local`) overwrites the local tag — compose
declares both `image:` and `build:` for each service, so it builds when
the image isn't present locally and pulls when you ask it to.

---

## Deploying the worker and API server separately

Everything above can also run as: one (or a small fixed pair behind a load
balancer) `codexec-api` host, and a separate, independently-sized pool of
`codexec-worker` hosts that scales with submission volume. Nothing in
either binary assumes they share a machine — they only share Postgres and
NATS — but a few things that are easy to overlook on a single box become
load-bearing once you split them out.

### Deploying Postgres and NATS separately

Both tiers (API and worker) connect to the same `DATABASE_URL` and
`NATS_URL`; run these on their own host(s) (or a managed Postgres/NATS
service), not colocated with either tier:

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
- **Configuration lives entirely in the two connection strings.** Neither
  `codexec-api` nor `codexec-worker` has any other Postgres/NATS-specific
  config beyond `DATABASE_URL`, `NATS_URL`, and `DB_POOL_SIZE` (shared,
  see `crates/codexec-common/src/config.rs`) — so once Postgres/NATS are
  reachable and authenticated, pointing either binary at a standalone
  cluster instead of `docker-compose.yml`'s bundled instances is a
  same-shape config change, nothing structural.
- **Migrations still run from `codexec-api`, never the worker** (see
  [Database migrations](#database-migrations)) — with a standalone
  Postgres, make sure whichever `codexec-api` instance starts first (or
  runs `sqlx migrate run` manually) has a role with schema-modification
  rights; the worker's DB role only ever needs to read/write rows, never
  run DDL.

### The API host

Nothing execution-related applies here — `codexec-api` never touches
`runc` or runs submissions, so it can live on a small, plain host (or
container) with just the binary, `DATABASE_URL`, `NATS_URL`, and
`ADMIN_API_TOKEN`. The `codexec/codexec-api` image (see
[Running in a Dockerized environment](#running-in-a-dockerized-environment))
is exactly that and needs no privileges, so this tier is the one that
drops cleanly onto whatever container platform you already run. It's
stateless request/response, so it scales horizontally the ordinary way
(multiple instances behind a load balancer, no session affinity needed) if
request volume ever warrants it — that's a much less interesting scaling
problem than the worker fleet below, since none of the actual submission
execution happens here.

### The worker fleet

Each worker host still needs everything from
[Prerequisites](#prerequisites)/[Configure](#configure) that's *local to
execution*: `runc`, `skopeo`/`umoci`, cgroup v2, a `WORKSPACE_ROOT`
directory, and every plugin image already present in *that host's own*
`IMAGE_CACHE_ROOT` (see [Register the language](#register-the-language)) —
the image cache is per-host, never shared, so this doesn't get easier by
adding more hosts, it gets repeated on each one. It does **not** need
`ADMIN_API_TOKEN`, `API_BIND_ADDR`, or anything else API-specific.

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
scale (pulling from each host's own local Docker daemon per
[Register the language](#register-the-language) doesn't scale to N hosts):

**Push plugin images to a real registry, and use fully-qualified
references in every `plugin.toml`.** Stand up a registry reachable from
every worker host (a self-hosted `registry:2`, or ECR/GCR/Docker
Hub/GHCR/etc.), push each built plugin image there, and set
`image.reference` to the fully-qualified pushed name, e.g.:

```toml
[image]
reference = "registry.internal.example.com/codexec/mylang:1.0.0"
```

This means `codexec-plugin-cli register`'s built-in pre-pull (the default
`--source registry`) just works standalone on every host — no per-host
Docker daemon needed at all once images are pushed.

With that in place, provisioning a worker host becomes: install
`runc`+`skopeo`+`umoci`, then pull every currently-active language's image
before `codexec-worker` starts accepting work. A boot-time script
(cloud-init user-data, or a systemd `ExecStartPre=`) covers both a fresh
instance joining the pool and re-running it manually across the fleet
right after you register a new plugin, since new instances get it from the
registry automatically, but *already-running* instances only get the new
image once this has run on them too.
`codexec-plugin-cli pull-image` does the pull without needing a full
plugin manifest or DB credentials on the provisioning host — just an image
ref:

```bash
#!/usr/bin/env bash
# provision-worker-images.sh — pull every active language's image into
# this host's image cache. Run at boot, and again on existing hosts
# whenever a new plugin is registered.
set -euo pipefail
API_URL="${CODEXEC_API_URL:-https://api.internal.example.com}"

curl -sf "$API_URL/admin/languages" -H "Authorization: Bearer $ADMIN_API_TOKEN" \
  | jq -r '.[].image_ref' \
  | sort -u \
  | while read -r ref; do
      echo "pulling $ref..."
      codexec-plugin-cli pull-image --image-ref "$ref"
    done
```

(A fleet-wide command runner — SSM Run Command, Ansible, a small
orchestration tool, whatever you're already using — is what actually
re-runs this on already-live instances; nothing in `codexec-worker` pushes
new images to running hosts for you. This script is also a good candidate
to run with a
[downloaded `codexec-plugin-cli` binary](#downloading-prebuilt-binaries-from-github-releases)
rather than a full source checkout on every worker host.)

### Autoscaling signal

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
resources — keep `ENGINE_TOTAL_CPU_CORES`/`ENGINE_TOTAL_MEMORY_MB` (see
[Configure](#configure)) sized to what the host actually has, since that's
what the exec engine uses to admit or queue new containers locally,
independent of whatever autoscaler is adding more hosts.

### Running the worker fleet inside containers

Everything above assumes worker hosts are plain Linux VMs, matching the
runc-on-bare-metal setup from [Prerequisites](#prerequisites). If you
instead run `codexec-worker` itself inside a container (e.g. Kubernetes
pods, one runc+skopeo+umoci+codexec-worker per pod, autoscaled via
HPA/KEDA on the same JetStream metric from
[Autoscaling signal](#autoscaling-signal)), you're back in the
nested-cgroup situation the dev `docker-compose.yml` `worker` service
exists for — you'll need that same cgroup `subtree_control` delegation
step from `docker/worker-entrypoint.sh`, a real init process as PID 1
(`tini`, same as the dev image — without one, `runc create`/`start`'s
reparented container-init helper is never reaped once the container
process exits, becoming a zombie that holds its cgroup open and confuses
`runc`'s own state reporting), a `privileged: true`-equivalent pod security
context, and per-pod-not-per-node image provisioning (the script from
[The worker fleet](#the-worker-fleet), run as each pod's init container
instead of at VM boot, since a fresh pod means a fresh, empty image cache
every time).

---

## Downloading prebuilt binaries from GitHub Releases

All three binaries — `codexec-api`, `codexec-worker`, `codexec-plugin-cli`
— are published the same way: as standalone Linux binaries (x86_64 and
aarch64, the two architectures codexec actually runs on, matching
`docker/*.Dockerfile`), attached to a GitHub Release whenever a version tag
(`vX.Y.Z`) is pushed, via `.github/workflows/release.yml`. A downloaded
binary is a drop-in replacement for `./target/release/<binary>` everywhere
else in this document — every env var, systemd unit, and host prerequisite
described elsewhere is unchanged; this only replaces the "compile it with
`cargo`" step, per binary, independently for each of the three.

No release published yet for your fork? The workflow also runs on-demand
(`workflow_dispatch` from the Actions tab) against any branch, so you can
sanity-check the build without cutting a real tag — but a `release` (the
GitHub Release itself, with downloadable assets) is only created when the
trigger is an actual `vX.Y.Z` tag push.

### Download a binary

Release assets are named without a version, so the
`.../releases/latest/download/...` URL always resolves to the newest
build — no need to look up a version number first. Repeat for each binary
you want (`codexec-api`, `codexec-worker`, `codexec-plugin-cli`):

```bash
BINARY=codexec-api   # or: codexec-worker / codexec-plugin-cli

ARCH="$(uname -m)"   # x86_64 or aarch64
case "$ARCH" in
  x86_64)  TARGET=x86_64-unknown-linux-gnu ;;
  aarch64) TARGET=aarch64-unknown-linux-gnu ;;
  *) echo "no prebuilt binary for $ARCH - see 'Building from source instead' below" >&2; exit 1 ;;
esac

BASE_URL="https://github.com/AdarshMishraji/Codexec/releases/latest/download"
curl -fsSL -o "${BINARY}.tar.gz" "$BASE_URL/${BINARY}-${TARGET}.tar.gz"
curl -fsSL -o "${BINARY}.tar.gz.sha256" "$BASE_URL/${BINARY}-${TARGET}.tar.gz.sha256"

# verify before running anything you just downloaded off the internet
sha256sum -c "${BINARY}.tar.gz.sha256"

tar xzf "${BINARY}.tar.gz"
sudo install -m 0755 "$BINARY" "/usr/local/bin/$BINARY"
```

### Download the frontend

The built React frontend is published the same way, as a single
architecture-independent asset (`frontend-dist.tar.gz`, built by a separate
`build-frontend` job in the same workflow — no Node toolchain needed on
your end):

```bash
BASE_URL="https://github.com/AdarshMishraji/Codexec/releases/latest/download"
curl -fsSL -o frontend-dist.tar.gz "$BASE_URL/frontend-dist.tar.gz"
curl -fsSL -o frontend-dist.tar.gz.sha256 "$BASE_URL/frontend-dist.tar.gz.sha256"
sha256sum -c frontend-dist.tar.gz.sha256

mkdir -p frontend-dist && tar -C frontend-dist -xzf frontend-dist.tar.gz
```

Point `STATIC_ASSETS_DIR` (see [Configure](#configure)) at wherever you
extracted it — e.g. `STATIC_ASSETS_DIR=$(pwd)/frontend-dist`.

### What each binary still needs

Downloading skips only the Rust build — every other prerequisite from
earlier in this document still applies, and differs per binary:

- **`codexec-api`** needs configuration (`DATABASE_URL`, `NATS_URL`,
  `ADMIN_API_TOKEN` — see [Configure](#configure)) and, separately, the
  built frontend on disk at `STATIC_ASSETS_DIR`. Unlike the old
  embedded-HTML version of this page, a downloaded `codexec-api` binary
  by itself has **no UI at all** — `/`, `/admin`, and `/docs` all 404 — the
  JSON API (`/submissions`, `/languages`, `/stats`, `/admin/*`) works fine
  regardless. Get the frontend either by building it yourself (see
  [Build](#build)) or by downloading the `frontend-dist.tar.gz` release
  asset below. It never touches `runc`, so aside from the frontend, a
  downloaded binary is otherwise the whole deployment for this tier.
- **`codexec-worker`** still needs `runc`, `skopeo`, `umoci`, and cgroup v2
  on the host, plus `IMAGE_CACHE_ROOT`/`RUNC_ROOT`/`WORKSPACE_ROOT` set up
  exactly as in [Prerequisites](#prerequisites)/[Configure](#configure) — a
  downloaded binary is a substitute for `cargo build`, not for any of that
  host setup.
- **`codexec-plugin-cli`** needs `skopeo`/`umoci` locally too, but only for
  its own `register`/`pull-image` subcommands (the ones that pull an
  image); `list`/`activate`/`deactivate` only need `DATABASE_URL`/
  `NATS_URL` reachable. See [Configure the CLI](#configure-the-cli) below.

### Configure the CLI

`codexec-plugin-cli` specifically is worth calling out on its own: unlike
the server and worker, it only ever needs network access to Postgres and
NATS (plus `skopeo`/`umoci` locally, for `register`/`pull-image`), not a
colocated `codexec-api` or `codexec-worker` process — which makes it a
reasonable thing to install directly on an operator's own laptop, a CI
runner, or a provisioning script host, on its own, without either server
process anywhere nearby. Same environment variables as everywhere else in
this document, since it's the same `codexec-common` config loading
underneath — either export them in your shell, or drop a `.env` next to
wherever you run it from (loaded automatically via `dotenvy`):

```bash
export DATABASE_URL="postgres://codexec:codexec@your-postgres-host:5432/codexec"
export NATS_URL="nats://your-nats-host:4222"
export IMAGE_CACHE_ROOT="/var/lib/codexec/images"   # only needed on a host that also runs codexec-worker
```

If you're only ever running `register --source docker-daemon`/`pull-image`
against a remote worker's image cache path (e.g. from a provisioning
script that runs *on* each worker host, as in
[The worker fleet](#the-worker-fleet)), `IMAGE_CACHE_ROOT` must match that
worker's own value. If you're running it purely against the admin HTTP API
instead of talking to Postgres/NATS directly, you don't need
`DATABASE_URL`/`NATS_URL`/`IMAGE_CACHE_ROOT` at all for the `curl`-based
registration flow shown in
[Register the language](#register-the-language) — only for the CLI's own
`register`/`list`/`activate`/`deactivate`/`pull-image` subcommands, which
talk to Postgres/NATS directly rather than going through `codexec-api`.

### Verify

```bash
codexec-api --help              # confirms the binary itself runs; it needs its env to actually start
codexec-worker --help
codexec-plugin-cli --version
codexec-plugin-cli list         # confirms DATABASE_URL is reachable and correct
```

### Building from source instead

Prebuilt binaries only cover Linux x86_64/aarch64. For any other platform
— macOS, say, to run `codexec-plugin-cli` from a laptop against a remote
Postgres/NATS, since `skopeo`/`umoci` are both available via Homebrew
there too (`codexec-api`/`codexec-worker` are Linux-only regardless, since
the worker's whole purpose is a Linux-only sandboxing mechanism) — or if
you just don't want to trust a downloaded binary, build any of the three
the normal way (see [Build](#build)):

```bash
cargo build --release -p codexec-plugin-cli   # or -p codexec-api / -p codexec-worker
./target/release/codexec-plugin-cli --version
```
