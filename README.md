# <img src="frontend/public/icon.png" alt="" width="32" align="center" /> Codexec

A code execution platform: submit source code in
any registered language, get back stdout/stderr/exit code plus **actual**
CPU time and peak memory consumed, with independently-enforced CPU-time,
CPU-rate, and memory limits. No language is built in — every language is a
plugin (an OCI image + a small manifest), added at runtime through an API
or a UI, never by editing code.

```
POST /submissions  { "language": "python3", "source_code": "print('hi')" }
  → 202 { "id": "...", "status": "queued" }
GET  /submissions/{id}
  → 200 { "status": "completed", "stdout": "hi\n",
           "cpu_time_used_ms": 8.2, "memory_used_kb": 4304, ... }
```

Live, running instance ships with:
- **`/`** — a public stats dashboard (throughput, latency percentiles, per-language/status/verdict breakdowns).
- **`/admin`** — a token-gated portal to register/activate/delete plugins, issue/revoke API keys, and a **Playground** to run submissions from the browser.
- **`/docs`** — full REST API reference (every endpoint, every field, every status code).

**→ [DEPLOYMENT.md](DEPLOYMENT.md) has the actual deployment instructions** (Docker Compose quick start, bare-metal Linux VM setup, splitting API/worker across an autoscaling fleet). This file is about how the system is built and why.

---

## How this works

### The three moving pieces

```
Client
  │  1. POST /submissions
  ▼
codexec-api  ──▶  Postgres: INSERT submission, status = 'queued'
  │
  │  2. publish a pointer {submission_id, language_slug} — never the source code
  ▼
NATS JetStream (durable work-queue stream)
  │
  │  3. pull (durable consumer, load-balanced across every running worker)
  ▼
codexec-worker
  │  4. claim:  UPDATE submissions SET status='processing' WHERE status='queued'
  │  5. execute via runc, read real cgroup stats
  │  6. write back the result, THEN ack the message
  ▼
runc sandbox (cgroup v2)
  cpu.max (rate cap) · cpu.stat polling (CPU-time budget) · memory.max (hard cap)

Meanwhile, the client polls:
Client  ──▶  GET /submissions/{id}  ──▶  codexec-api  ──▶  Postgres (same row)
```

**`codexec-api`** (axum) validates a submission, writes a `queued` row to
Postgres, and publishes a small pointer (`{submission_id, language_slug}` —
never the source code itself) to a NATS JetStream work-queue stream. It
returns `202` immediately; there is no synchronous "run and wait" mode.

**`codexec-worker`** pulls from a durable JetStream consumer, claims a
submission with one conditional SQL update
(`WHERE id = $1 AND status = 'queued'`), hands it to the execution engine,
and writes the result back before acking. That claim step is the entire
correctness story: JetStream is at-least-once, so the same message can be
redelivered — the conditional update means only one worker ever wins a
given submission, and a crashed worker's claim expires and gets
automatically reclaimed by whichever worker picks up the redelivery. API
and worker never talk to each other directly; Postgres and NATS are the
only integration points, so either side can be scaled, restarted, or
crashed independently.

**The execution engine** (`codexec-exec-engine`) runs each submission as a
plain `runc` subprocess — no `containerd`, no Docker daemon, no long-lived
sandboxing service. A plugin's OCI image is pulled once at registration
time (`skopeo` + `umoci`, unpacked to a plain rootfs directory) and shared
read-only across every future submission for that language; a submission
only ever gets its own thin, per-run workspace bind-mounted in. Every
resource limit maps to a real kernel primitive, read back from the cgroup
after the run — not estimated:

| Limit | Enforcement | Kernel primitive |
|---|---|---|
| `cpu_limit_cores` | Rate cap — how much concurrent CPU the sandbox can draw | cgroup v2 `cpu.max` (CFS quota/period), enforced by the scheduler, no polling |
| `cpu_time_limit_ms` | Total CPU-time budget | cgroup v2 `cpu.stat`'s `usage_usec`, polled every 25ms, killed on breach — cgroups have no native "kill after N CPU-seconds" primitive |
| `memory_limit_kb` | Hard memory ceiling | cgroup v2 `memory.max`; an OOM kill is disambiguated from a plain crash via `memory.events`' `oom_kill` counter |
| (internal) `wall_time_limit_ms` | Safety net, not user-facing | `3× cpu_time_limit_ms + 2s`, capped — catches a program that blocks/sleeps instead of burning CPU, which neither limit above would otherwise bound |

A submission's compile step and run step happen inside **one** container
(a generated wrapper script sequences them and marks the phase boundary),
so there's no artifact hand-off problem and no doubled container-lifecycle
overhead per submission.

### The plugin system

A language is a row in Postgres (image reference, compile/run argv,
default and max limits) plus an already-pulled image. Nothing about a
language is hardcoded in the binary. Three ways to register one:

- `codexec-plugin-cli register --manifest plugins/python3/plugin.toml` (also pulls the image via `skopeo`/`umoci`).
- `POST /admin/languages` with the same manifest shape, from any automation — the database write happens immediately, and every running worker independently notices and pulls the image itself in the background (`ensure_image_cached`, pushed via a NATS control-subject notification, with a periodic full re-check as a fallback for a dropped message) — no CLI access to a worker host required.
- The Admin Portal's "Add Plugin" form, with a **template picker** that pre-fills the form from one of the 12 plugins already shipped in this repo (`c`, `cpp`, `csharp`, `dart`, `go`, `java`, `javascript`, `kotlin`, `python3`, `rust`, `swift`, `typescript` — see `plugins/`), each with a real, tested Dockerfile and manifest.

Activating/deactivating/deleting a plugin, and every submission made
against it, are all tracked with real history — deleting a plugin that's
ever been submitted to is refused (`409 conflict`) rather than silently
orphaning historical rows.

### Everything else

API keys gate `/submissions*` (opaque tokens, SHA-256 hashed at rest, shown
in full exactly once at creation); a separate static admin token gates
`/admin/*`. Grading is optional and simple: give a submission an
`expected_output` and its `verdict` becomes `accepted`/`wrong_answer` on an
exact match (trailing-whitespace-normalized) once it completes — no
partial credit, no pluggable checkers.

---

## What makes this different

Compared to the two most common self-hostable code-execution engines:

| | **codexec** | **Judge0** | **Piston** |
|---|---|---|---|
| Isolation | `runc` directly against cgroup v2, no daemon | `isolate` (namespaces/cgroups sandbox built for IOI) | A real Docker container per execution |
| Needs a container daemon at run time? | No — images are pre-unpacked to a plain rootfs | No | Yes — the Docker daemon itself |
| API ↔ execution coupling | Fully decoupled: HTTP API and worker only share Postgres + a NATS queue, independently scalable/deployable | Traditionally one host running the API and `isolate` together | API and Docker daemon on the same host |
| CPU limit model | **Two independent knobs**: a hard CPU-*rate* cap and a separate CPU-*time* budget, both kernel-enforced/read back from the cgroup | A single wall/CPU time limit | A single timeout |
| Adding a language | Any OCI image + a manifest, registered at runtime via API/UI/CLI — no fork, no rebuild | A fixed, curated compiler list baked into the isolate box | A curated package catalog you install per-instance |
| Ops surface | Ships a live dashboard, an admin UI (with a request Playground), and generated API docs — a React SPA served same-origin by `codexec-api` itself, no separate proxy or process | Primarily an API; a bundled web UI exists separately | API/CLI only |
| Result data | Real cgroup-measured CPU time *and* peak memory, both persisted per submission | CPU/wall time and memory reported similarly, via `isolate`'s own accounting | Minimal — mainly stdout/stderr/exit code |

The throughline: codexec treats "queue-decoupled, independently scalable
API/worker" and "two separate, kernel-enforced CPU controls" as first-class
architectural requirements rather than something layered on top — both
were locked in from the initial design, not retrofitted.

---

## Project layout

```
crates/
  codexec-common        DB models, config loading, grading, API-key hashing, plugin-registry SQL
  codexec-exec-contract  The ExecutionRequest/ExecutionOutcome/ExecutionEngine trait — the thin boundary between worker and engine
  codexec-exec-engine    runc-backed engine: image pull/unpack, OCI spec, cgroup stats, lifecycle, classification
  codexec-api            axum HTTP server — submissions, admin routes, and serving the built frontend
  codexec-worker         NATS consumer: claim/lease/execute/write-back loop, plugin registry cache
  codexec-plugin-cli     Operator CLI: register/activate/deactivate a plugin from a manifest
frontend/                React (Vite + TypeScript) SPA — dashboard, admin portal, API docs. Built to
                         static files and served by codexec-api itself (tower-http's ServeDir), same
                         origin, no separate reverse proxy or process.
plugins/                 One plugin.toml per shipped language, usually alongside its own Dockerfile
                         (a few, like python3, just point at a stock public image instead)
migrations/              sqlx migrations (source of truth for the schema)
docker/                  Dockerfiles for the two deployable images (api/worker) + docker-compose's worker entrypoint
```

For a deeper dive into the worker's own internals specifically (the
claim/lease state machine, the registry cache, the image-pull handoff),
see `.claude/skills/codexec-worker-internals/` in this repo.

## Deploying it

See **[DEPLOYMENT.md](DEPLOYMENT.md)** — covers the Docker Compose quick
start, a from-source bare-metal Linux VM setup, and splitting the API and
an autoscaling worker fleet across multiple hosts.
