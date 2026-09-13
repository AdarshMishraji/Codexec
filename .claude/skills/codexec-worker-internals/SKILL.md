---
name: codexec-worker-internals
description: Deep reference for crates/codexec-worker - the NATS claim/lease/execute/write-back pipeline, the plugin registry cache with its worker-side image auto-pull, and the handoff into the runc execution engine. Load before modifying, debugging, or extending the worker.
---

# codexec-worker internals

`codexec-worker` is the only component in this system that actually executes
untrusted submitted code. Everything else — `codexec-api`, the admin portal,
the dashboard — only ever writes a `queued` row to Postgres and a pointer to
NATS JetStream; the worker is what turns that into a real sandboxed process
run and a final result. If this crate has a bug, submissions silently never
complete, complete with wrong results, or (worse) run without their resource
limits enforced — so its correctness is disproportionately load-bearing for
the whole project succeeding at its stated goal ("isolation with all the
limiting parameters — memory, CPU, etc").

This skill is the map. For the exhaustive, function-by-function explanation
(every function in `main.rs`, `config.rs`, `registry.rs`, plus the exact
boundary contracts it depends on), see **`reference/functions.md`** — load it
whenever you need to reason about one specific function rather than the
system as a whole.

## Why the worker exists as a separate process

Requirement 2 of this project (see the original design) is that the API
server and the worker are independently deployable and never talk to each
other directly — only through Postgres (system of record) and NATS
JetStream (work queue). That's not a style preference: it means the API can
stay up and keep accepting submissions even if every worker is down (they
just queue), and workers can be scaled, restarted, or crashed individually
with zero coordination with the API process. The worker crate is the half of
that contract that *consumes* the queue.

## The pipeline, end to end

One NATS message in, one Postgres row updated, one ack out. In order, and
**why** each step exists (this is the part most worth understanding before
touching `handle_message` — see `reference/functions.md` for the literal
branch-by-branch walkthrough):

1. **Pull a message from the durable JetStream consumer.** The payload is
   just `{submission_id, language_slug, enqueued_at}` — a *reference*, never
   the source code itself, so Postgres stays the single source of truth and
   a redaction there can never be undone by a stale copy sitting in the
   queue.
2. **Poison-message guard.** An undecodable payload, or one past
   `NATS_MAX_DELIVER` attempts, is acked away (with the row marked
   `internal_error` if it's not already terminal) instead of being retried
   forever or crashing the worker.
3. **Claim the row with a single conditional `UPDATE ... WHERE status =
   'queued'`.** This is the whole idempotency story in one SQL statement: if
   two workers (or two redeliveries) race on the same message, exactly one
   `UPDATE` matches and returns a row; the other gets zero rows back and
   takes one of several safe fallback paths (already terminal → ack and
   move on; still processing under a live lease → NAK and retry later; lease
   expired → the prior claimant probably crashed, so reclaim it).
4. **Look up the plugin.** Checked against the in-memory registry cache
   first (fast, no DB round trip on the hot path), with a direct one-off
   Postgres fallback (`fetch_fresh`) if the cache hasn't caught up yet — so
   a plugin activated moments ago doesn't spuriously fail.
5. **Hand off to the execution engine.** `ExecutionEngine::execute` is
   `async` but **infallible** — it always returns an `ExecutionOutcome`
   value, never a `Result`/error. A containerd-equivalent outage becomes
   `InternalError { retryable: true }`, not a panic or an unhandled `?`. This
   is what lets `handle_message` never crash the whole worker process over
   one bad submission.
6. **Map the outcome to a database write.** Exit-code-vs-crash
   classification (`runtime_error` vs `completed`) and grading (`accepted` /
   `wrong_answer`, only when `expected_output` was given *and* the run
   actually completed) both happen here, in the worker — not the engine,
   which has no concept of either.
7. **Write back, then ack — never the other way round.** The message is
   only acked *after* the Postgres write commits. If the process dies
   between execution and write-back, the message simply gets redelivered
   once the lease/ack-wait expires, and step 3's conditional claim makes
   that redelivery safe rather than a double-charge.

## Concurrency model

Two independent layers, easy to conflate:

- **Process-level fan-out**: `main` pulls messages in a loop and
  `tokio::spawn`s a fresh `handle_message` task per message, gated by a
  `tokio::sync::Semaphore` sized to `WORKER_CONCURRENCY`. This bounds how
  many submissions this one worker process handles at once.
- **Resource-weighted admission inside the engine** (`EngineConfig.
  total_cpu_cores` / `total_memory_bytes`, enforced by
  `codexec-exec-engine`, not this crate): a flat count-based semaphore alone
  is insufficient because submissions request different CPU/memory amounts
  — N concurrent cheap jobs and N concurrent expensive ones are very
  different loads. This crate only sets the ceilings via `EngineConfig`; it
  doesn't implement the weighted admission logic itself.

## The plugin registry cache

`registry.rs`'s `PluginRegistry` exists purely so `handle_message` isn't
doing a `SELECT` against `languages` for every single message — it keeps an
`Arc<RwLock<HashMap<slug, Language>>>` in memory, refreshed two ways: a
near-instant push via NATS core pub/sub (`codexec.control.plugin_updated`,
fired by `codexec-api` on every register/activate/deactivate/delete) and a
60-second periodic full refresh as a fallback for a dropped
at-most-once pub/sub message.

It was later extended with **`ensure_image_cached`**: a plugin registered
purely through the admin API (as opposed to `codexec-plugin-cli register`)
only ever writes a database row — the API server has no reason to run
`skopeo`/`umoci` itself, and may not even share a filesystem with any
worker. So every worker checks, on every registry refresh, whether the
image it just learned about is already unpacked locally
(`image::ensure_present`); if not, it pulls it itself in a background task
(`image::pull_and_unpack`, registry source only). Without this, an
admin-API-registered plugin would look perfectly active in the database
while failing every real submission with `ImageNotFound`. See
`reference/functions.md` for the exact function.

## Configuration surface (`config.rs`)

All read once at startup via `WorkerConfig::from_env()`, no live reload:

| Env var | Default | Controls |
|---|---|---|
| `WORKER_CONCURRENCY` | `4` | Process-level semaphore permits — max submissions this worker executes at once |
| `WORKER_CONSUMER_NAME` | `workers-generic` | Durable JetStream consumer name; workers sharing a name load-balance automatically |
| `WORKER_CONSUMER_FILTER_SUBJECT` | `codexec.submissions.>` | Narrow this (e.g. to one language's subject) to run a dedicated pool |
| `NATS_ACK_WAIT_SECS` | `120` | Also used as this worker's own claim-lease duration (`lease_expires_at`) |
| `NATS_MAX_DELIVER` | `5` | Delivery attempts before a message is treated as poison |
| `IMAGE_CACHE_ROOT` | `/var/lib/codexec/images` | Where unpacked plugin rootfs directories live |
| `RUNC_ROOT` | `/run/codexec/runc` | `runc --root`, dedicated so it can't collide with e.g. Docker's own |
| `WORKSPACE_ROOT` | `/var/lib/codexec/workspaces` | Per-submission scratch directories |
| `CGROUP_ROOT` | `/sys/fs/cgroup` | Where the engine reads cgroup v2 stats from |
| `ENGINE_TOTAL_CPU_CORES` | `4.0` | Engine-wide admission-control budget — set below true host capacity |
| `ENGINE_TOTAL_MEMORY_MB` | `4096` | Same, for memory |

## When things go wrong (symptom → where to look)

- **Submissions stuck in `queued` forever, no worker logs at all** → check
  the worker process is actually connected to the same NATS/`SUBMISSIONS`
  stream and its consumer's `filter_subject` actually matches
  `codexec.submissions.{language_slug}`.
- **A specific submission stuck in `processing` past its `lease_expires_at`
  with no progress** → the worker that claimed it likely crashed mid-run;
  the *next* redelivery's reclaim branch in `handle_message` should recover
  it automatically once the ack-wait/lease elapses — if it doesn't, check
  the ack-wait vs lease duration are actually the same value (they're meant
  to be: both come from `NATS_ACK_WAIT_SECS`).
- **`internal_error` with `"language ... no longer active"` right after
  activating a plugin** → the cache genuinely lagged both the control
  message and the periodic refresh; this should self-heal within 60s, and
  `fetch_fresh`'s fallback should mean it basically never happens in
  practice — if it's frequent, check the NATS control subject subscription
  is actually connected (`spawn_control_subscriber`).
- **A plugin added via the admin portal 404s every submission with
  `ImageNotFound`-flavored `internal_error`** → check the worker's logs for
  `"image not yet cached on this worker, pulling in background"` /
  `"automatic image pull failed"` from `ensure_image_cached` — the pull
  itself may be failing (bad image ref, no registry access from this host).
- **Same submission executed twice / double-charged** → this should be
  structurally impossible given the conditional-`UPDATE` claim + ack-after-
  write-back ordering; if you ever see it, the bug is almost certainly a
  change that acks before the write-back commits, or that removed the
  `WHERE status = 'queued'` / `WHERE status = 'processing'` guard from one
  of the claim/reclaim queries.
