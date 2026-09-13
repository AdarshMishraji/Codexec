# codexec-worker — every function, in detail

Scope: `crates/codexec-worker/src/{main,config,registry}.rs` in full, plus
the exact boundary contracts this crate depends on but doesn't own
(`codexec-exec-contract`'s trait/types, `codexec_common::grading::grade`,
and the two `codexec_exec_engine::image` functions `registry.rs` calls
directly). Signatures are quoted verbatim from source; line numbers will
drift as the crate evolves, so treat this as "what each function does and
why", not a byte-for-byte mirror.

---

## `src/main.rs`

### Module constants

```rust
const WALL_TIME_MULTIPLIER: u64 = 3;
const WALL_TIME_FIXED_OVERHEAD_MS: u64 = 2000;
const PLATFORM_MAX_WALL_TIME_MS: u64 = 120_000;
const MAX_OUTPUT_BYTES: u64 = 1_048_576;
```

`wall_time_limit_ms` is deliberately **not** a user-facing submission field
(see `codexec-api`'s submission validation) — it's a safety-net ceiling
computed here, worker-side, so a program that *blocks/sleeps* instead of
burning CPU still gets killed eventually, which neither the CPU-rate cap
nor the CPU-time poll would otherwise bound. `MAX_OUTPUT_BYTES` caps how
much stdout/stderr the engine will buffer per run regardless of what the
plugin or submission requests.

### `build_engine(config: &WorkerConfig) -> Arc<dyn ExecutionEngine>`

Translates the worker's flat `WorkerConfig` into the execution engine's own
`EngineConfig` (paths for the image cache/runc state/workspaces/cgroups,
the admission-control CPU/memory budget, and a few fixed tuning constants:
10s default compile timeout, 2s kill grace period, 25ms CPU-poll interval)
and constructs the one concrete engine implementation,
`codexec_exec_engine::RuncExecutionEngine`, behind the `ExecutionEngine`
trait object. Called exactly once at startup; the returned `Arc` is cloned
into every spawned `handle_message` task afterward. Swapping in a different
engine implementation (e.g. a test double) means changing only this one
function — nothing else in `main.rs` knows the concrete type.

### `build_request(run_id: Uuid, submission: &Submission, language: &Language) -> ExecutionRequest`

Assembles the engine-facing `ExecutionRequest` from a claimed `Submission`
row plus its resolved `Language` plugin row. The one piece of real logic
here is the wall-time derivation:

```rust
let wall_time_limit_ms = ((submission.cpu_time_limit_ms as u64) * WALL_TIME_MULTIPLIER
    + WALL_TIME_FIXED_OVERHEAD_MS)
    .min(PLATFORM_MAX_WALL_TIME_MS);
```

i.e. "3x the requested CPU-time budget, plus 2 seconds of fixed overhead,
capped at 120 seconds no matter what" — generous enough that a legitimately
CPU-bound, I/O-light program never trips it before the CPU-time poll would
catch it, but finite enough that a submission that just calls `sleep()`
forever still terminates. Everything else is a direct field-for-field copy,
with `compile_cmd`/`run_cmd` converted from the DB's stored JSONB argv
arrays back into `Vec<String>` via `codexec_common::models::parse_argv`.

### `struct WriteBack`

Not a function, but worth naming: the intermediate shape both
`map_outcome` produces and `write_back` consumes. It exists so the mapping
logic (pure, easily testable, no I/O) is fully decoupled from the actual
`UPDATE` statement — `map_outcome` never touches Postgres, `write_back`
never inspects an `ExecutionOutcome`.

### `map_outcome(outcome: ExecutionOutcome, expected_output: Option<&str>) -> WriteBack`

The single place that turns an engine result into the two things the rest
of the system cares about: a `SubmissionStatus` and, conditionally, a
`SubmissionVerdict`. Match arms, one per `ExecutionOutcome` variant:

- **`Completed`**: `exit_code == 0` → `SubmissionStatus::Completed`;
  anything else → `SubmissionStatus::RuntimeError`. This is a deliberate
  design choice documented right above the function: *the engine doesn't
  distinguish "crashed" from "a clean nonzero exit"* — that one `if`
  belongs at this layer, not inside the engine, because it's a
  judge-domain concept, not an execution-domain one. Grading
  (`codexec_common::grading::grade`) only ever runs when the status came
  out `Completed` *and* the submission actually carried an
  `expected_output` — a compile error, a runtime error, or a limit breach
  never gets a verdict, since there's no meaningful output to grade.
- **`CompileError`**: maps to `SubmissionStatus::CompileError`, with the
  engine's `stderr` landing in the row's `compile_output` field (not
  `stderr` — that column is reserved for the *run* phase's stderr).
- **`TimeLimitExceeded`** / **`MemoryLimitExceeded`**: map 1:1 to their
  matching `SubmissionStatus` variants. Note neither ever produces a
  `verdict` — a submission that got killed didn't produce a complete,
  gradeable run.
- **`InternalError`**: maps to `SubmissionStatus::InternalError`, capturing
  `message` into the row's `error_message`. The outcome's `retryable` flag
  is *not* consulted here — by the time `map_outcome` runs, the submission
  has already been claimed and executed once; retry policy for the queue
  layer is a separate concern handled by JetStream redelivery semantics,
  not by this write-back mapping.

### `async fn write_back(pool: &PgPool, id: Uuid, wb: WriteBack) -> Result<(), sqlx::Error>`

One `UPDATE submissions SET ...`, binding every `WriteBack` field plus
`finished_at = now()`/`updated_at = now()`, keyed on `id`. Deliberately the
*only* place in this crate that writes a submission's terminal fields —
`handle_message` never constructs that SQL itself, which keeps the
column list in exactly one place. Returns a plain `Result` (unlike almost
everything else in the message-handling path, which swallows errors into
best-effort fallbacks) because a failed write-back is the one failure mode
that must **not** be silently absorbed: see `handle_message`'s call site,
which deliberately does not ack the message if this fails.

### `async fn handle_message(msg, pool, registry, engine, worker_id, ack_wait_secs)`

The heart of the crate — one full submission lifecycle, spawned as an
independent task per NATS message. Walked through in the order it actually
executes:

**1. Payload decode.** `serde_json::from_slice::<serde_json::Value>` on the
raw payload, then pulls out `submission_id` as a `Uuid`. Either failure
(undecodable JSON, or a missing/non-UUID `submission_id`) is treated as a
**poison message**: logged, acked away immediately, function returns. There
is deliberately no retry here — a payload this malformed will never become
valid on redelivery.

**2. Max-delivery check.** `msg.info()?.delivered >= 5` (the literal ceiling
mirrors `NATS_MAX_DELIVER`'s default, but note this is a **hardcoded `5`**
in this check specifically, not read from `config.nats_max_deliver` — worth
knowing if you ever change `NATS_MAX_DELIVER` and expect this check to
follow). Once tripped, the row is force-marked `internal_error` (but *only*
if it isn't already one of the six terminal statuses — an `UPDATE ... WHERE
... status NOT IN (...)` guard prevents clobbering a result that actually
finished before the delivery count caught up) and the message is acked
away. This is the backstop against a message that keeps failing for a
reason redelivery can't fix.

**3. Claim.** The one `UPDATE` that makes the whole pipeline idempotent:

```sql
UPDATE submissions
SET status = 'processing', started_at = now(), worker_id = $1,
    lease_expires_at = now() + ($2 || ' seconds')::interval, updated_at = now()
WHERE id = $3 AND status = 'queued'
RETURNING *
```

If this returns a row, this worker now owns the submission outright for up
to `ack_wait_secs`. If it returns nothing, exactly one of four things is
true, distinguished by a follow-up `SELECT`:

  - **No row exists at all yet.** Not necessarily poison — `codexec-api`
    awaits the JetStream publish ack *before* committing its own INSERT
    transaction, so a worker can legitimately pull and try to claim a
    submission a few milliseconds before the row is visible to a *fresh*
    connection. Handled with a short `Nak(Some(250ms))` so JetStream
    redelivers shortly after the commit lands, rather than either treating
    this as permanent poison (losing the submission) or busy-looping.
  - **Row exists and is already terminal.** The ack was lost somewhere
    upstream but the work is already done — just ack it away, no re-run.
  - **Row exists, `status = 'processing'`, lease not yet expired.** Another
    live worker owns it right now — `Nak(Some(5s))` and back off; do **not**
    reclaim.
  - **Row exists, `status = 'processing'`, lease expired.** The prior
    claimant almost certainly crashed mid-execution. Re-claims via a second
    conditional `UPDATE ... WHERE status = 'processing'` (note: *not*
    `'queued'`, since the row never went back to `queued`). If even this
    reclaim loses a race (another worker beat it to the same reclaim), it
    backs off with the same short `Nak` and lets the *next* redelivery sort
    it out.
  - **Row exists, still `'queued'`.** The original claim UPDATE raced
    against the same INSERT-visibility window as the "no row yet" case and
    lost. `Nak(Some(250ms))` and retry via redelivery rather than silently
    dropping the submission.

**4. Language resolution.** `registry.get(slug)` (cache hit, the fast/
common path) with a `registry.fetch_fresh(slug)` fallback on a cache miss —
see the registry section below for why this two-tier lookup exists. If
*neither* finds an active language (the plugin was deactivated/deleted
after the submission was accepted), the row is written back as
`internal_error` with an explicit message and acked away — this is a
terminal outcome, not retried, since redelivery won't make a deactivated
plugin reappear.

**5. Execute.** `build_request` → `engine.execute(req).await` → `map_outcome`.
No error handling needed around the `execute` call itself — the trait
guarantees an `ExecutionOutcome` value no matter what happened inside the
engine (see the contract section below).

**6. Write back, then ack.** If `write_back` fails, the function returns
**without acking** — deliberately, so JetStream redelivers once the
ack-wait window elapses and the whole claim (step 3) runs again. If it
succeeds, `msg.ack()` — only now, after the commit, is the message
considered fully handled.

### `async fn main() -> anyhow::Result<()>`

Startup and the top-level pull loop:

1. Load `.env` (best-effort, ignored if absent) and init `tracing`.
2. `WorkerConfig::from_env()` — fails fast if a required var is missing/
   unparseable.
3. Open the Postgres pool (`PgPoolOptions::max_connections(config.common.
   db_pool_size)`).
4. Connect to NATS, wrap it in a JetStream context.
5. `get_or_create_stream` for `SUBMISSIONS` (subjects
   `codexec.submissions.>`, `WorkQueuePolicy`, file storage) — idempotent,
   safe even if `codexec-api` already created it.
6. `get_or_create_consumer` — a **durable pull consumer** named
   `config.consumer_name`, filtered to `config.consumer_filter_subject`,
   explicit ack policy, `ack_wait` = `NATS_ACK_WAIT_SECS`, `max_deliver` =
   `NATS_MAX_DELIVER`. Multiple worker processes sharing the same durable
   name load-balance pulls automatically — this is what lets you run N
   worker replicas with zero extra coordination code.
7. `PluginRegistry::load(...)` (initial full cache fill), then
   `spawn_control_subscriber` and `spawn_periodic_refresh` — both
   fire-and-forget background tasks that outlive this function.
8. `build_engine(&config)`.
9. A `Semaphore::new(config.worker_concurrency)` for process-level
   fan-out control (see the concurrency section of `SKILL.md`).
10. A `worker_id` string (`"{hostname}-{uuid_v4}"`) — written into every
    submission this process claims, for observability and for the lease
    reclaim logic to have a value to overwrite.
11. The pull loop: `consumer.messages().await?` yields an async stream;
    each item is either a `Message` (acquire a semaphore permit, clone the
    handful of `Arc`/`String` values, `tokio::spawn(handle_message(...))`)
    or an `Err` (logged, loop continues — a transient pull error doesn't
    kill the worker).

This function only returns on an unrecoverable startup failure (bad config,
can't reach Postgres/NATS) — once the loop starts, it runs until the
process is killed.

### `fn hostname() -> String`

`std::env::var("HOSTNAME")`, falling back to the literal string
`"worker"`. Purely cosmetic/diagnostic (feeds into `worker_id`) — not a
correctness-relevant function, but worth knowing it's an env var read, not
an actual `gethostname(2)` syscall, so it depends on the container/host
actually exporting `HOSTNAME` (true by default in Docker, not guaranteed
everywhere).

---

## `src/config.rs`

### `WorkerConfig::from_env() -> Result<Self, ConfigError>`

Pure env-var parsing, no I/O, no validation beyond type parsing (there is no
range/sanity checking here — e.g. `ENGINE_TOTAL_CPU_CORES=0` would parse
successfully and only misbehave much later, inside the engine's admission
control). See the table in `SKILL.md` for the full var list, defaults, and
what each one tunes. `common: CommonConfig` is shared with `codexec-api`
(`DATABASE_URL`, `NATS_URL`, `DB_POOL_SIZE`, `RUST_LOG`) and lives in
`codexec-common::config`, not this crate.

---

## `src/registry.rs`

### `struct PluginRegistry { inner: RwLock<HashMap<String, Language>>, pool: PgPool, image_cache_root: PathBuf }`

An in-memory, eventually-consistent mirror of `SELECT * FROM languages
WHERE is_active`, keyed by slug. Exists purely to keep `handle_message` off
the database for the extremely common "look up this submission's language"
step. "Eventually consistent" is load-bearing here: every lookup path in
this crate is written assuming the cache can be briefly stale, and has an
explicit fallback for that (see `get`/`fetch_fresh` below, and step 4 of
`handle_message` above).

### `async fn PluginRegistry::load(pool: PgPool, image_cache_root: PathBuf) -> Result<Arc<Self>, sqlx::Error>`

Constructs the registry and immediately calls `refresh_all()` once,
synchronously, before returning — so by the time `main` moves on to
building the consumer/engine, the cache is already warm. Wrapped in `Arc`
because it's shared across every spawned `handle_message` task plus the two
background refresh tasks.

### `async fn PluginRegistry::refresh_all(&self) -> Result<(), sqlx::Error>`

`SELECT * FROM languages WHERE is_active`, rebuilds the whole `HashMap` from
scratch under a single write-lock acquisition (not an incremental merge —
simpler, and cheap enough at the expected scale of "number of registered
languages", which is nowhere near performance-sensitive). After swapping in
the new map, calls `ensure_image_cached` once per language — so *every*
periodic refresh (see `spawn_periodic_refresh`) doubles as a chance to
notice and pull any image that's still missing. This is the fallback path
for "the control-subject notification about a new plugin got dropped."

### `async fn PluginRegistry::refresh_one(&self, slug: &str)`

The control-subject-driven counterpart to `refresh_all` — re-queries just
one row (`WHERE slug = $1 AND is_active`) instead of the whole table.
Three outcomes: found-and-active → insert/overwrite that one cache entry
and call `ensure_image_cached`; not-found-or-inactive → remove it from the
cache (covers both deactivation and the plugin never having existed); DB
error → logged and swallowed (a transient DB hiccup on a cache-refresh path
shouldn't take down the subscriber loop that called this). Never returns a
`Result` — by design, nothing upstream needs to react to a failed refresh
beyond what's already logged.

### `fn PluginRegistry::ensure_image_cached(&self, language: Language)`

Not async itself — it `tokio::spawn`s a background task and returns
immediately, so a slow or failed image pull never blocks the refresh loop
(control-subject handling, periodic refresh) from moving on to the next
language. Inside the spawned task: `image::ensure_present(cache_root,
&language.image_ref)` — if that succeeds, the image is already unpacked and
there's nothing to do. If it fails (`ImageNotFound`), logs an info line and
calls `image::pull_and_unpack(cache_root, &language.image_ref,
ImageSource::Registry, force: false)`, logging success or a warning on
failure. **Registry source only** — a custom image that only exists in
someone's local Docker daemon still needs an explicit
`codexec-plugin-cli register --source docker-daemon` run, since that's the
one case no worker can resolve unilaterally (it has no way to know *whose*
Docker daemon to pull from). This function is the entire reason
admin-API-registered plugins work end-to-end without ever invoking
`codexec-plugin-cli` — see `SKILL.md`'s "plugin registry cache" section for
the fuller story of why it needed to exist at all.

### `async fn PluginRegistry::get(&self, slug: &str) -> Option<Language>`

A plain read-lock `HashMap::get` + clone. The fast path used by
`handle_message` on every single submission. Returns `None` on a cache
miss — callers are expected to fall back to `fetch_fresh`, not to treat
`None` as "this language doesn't exist."

### `async fn PluginRegistry::fetch_fresh(&self, slug: &str) -> Option<Language>`

The slow-path fallback: a direct `SELECT * FROM languages WHERE slug = $1
AND is_active`, bypassing the cache entirely. Exists specifically so a
language activated moments ago (faster than the control-subject message
arrived, or during the window before the first periodic refresh) doesn't
spuriously fail a submission with `internal_error`. On a hit, it also
**backfills the cache** (`self.inner.write().await.insert(...)`) so the
*next* lookup for the same slug doesn't need to hit Postgres again. Errors
are swallowed to `None` (`.ok().flatten()`) — from the caller's point of
view, "DB error while double-checking" and "language genuinely doesn't
exist" are handled identically (both end in the submission failing as
`internal_error`), which is an acceptable simplification since this is
already the fallback-of-last-resort path.

### `fn spawn_control_subscriber(registry: Arc<PluginRegistry>, nats: async_nats::Client)`

Subscribes (plain NATS core pub/sub, not JetStream — this is a
cache-invalidation *hint*, not data of record, so at-most-once delivery is
fine) to `codexec.control.plugin_updated`. On every message, parses out a
`slug` field and calls `registry.refresh_one(slug)` — it never trusts
anything else in the payload (the `action` field `codexec-api` sends is
purely informational/for logs elsewhere; this subscriber only reacts to
*which* slug changed and always re-derives the truth from Postgres). Runs
forever in its own `tokio::spawn`; if the subscription itself fails to
establish, it logs an error and returns (the worker keeps running on
periodic-refresh-only cache updates, degraded but not down).

### `fn spawn_periodic_refresh(registry: Arc<PluginRegistry>, interval: Duration)`

A `tokio::time::interval` loop calling `registry.refresh_all()` every
`interval` (60 seconds, as wired in `main`) forever. The explicit fallback
for a dropped control-subject message — core NATS pub/sub has no
redelivery, so without this, a lost `plugin_updated` message could leave a
worker's cache permanently stale until process restart.

---

## Boundary contracts (owned by other crates, but essential to the worker)

### `codexec_exec_contract::ExecutionRequest` / `ExecutionOutcome` / `ExecutionEngine`

The thin interface between this crate's queue-consumer loop and whatever
actually runs code (`codexec-exec-engine`'s `RuncExecutionEngine` in
production). Deliberately dependency-free (no NATS/Postgres/containerd
types leak into it) so the worker and the engine can be reasoned about,
and tested, independently.

- **`ExecutionRequest`**: everything the engine needs for one run — argv
  commands (never a shell string; no template substitution, so there's no
  injection surface), the three user-facing limits
  (`cpu_time_limit_ms`/`cpu_limit_cores`/`memory_limit_kb`), the
  worker-derived `wall_time_limit_ms` safety net, and `max_output_bytes`.
- **`ExecutionOutcome`**: a five-variant enum (`Completed`, `CompileError`,
  `TimeLimitExceeded`, `MemoryLimitExceeded`, `InternalError`) — see
  `map_outcome` above for exactly how each becomes a `SubmissionStatus`.
- **`ExecutionEngine::execute(&self, req) -> ExecutionOutcome`**: `async
  fn`, and critically **infallible** — there is no `Result`, no `?`. Every
  failure mode the engine can hit, including a completely unreachable
  execution backend, is required to be represented as
  `ExecutionOutcome::InternalError { message, retryable }` rather than
  propagated as a Rust error. This is what lets `handle_message` call
  `engine.execute(req).await` with zero error-handling ceremony and still
  guarantee it never panics or hangs the task over one bad submission.

### `codexec_common::grading::grade(expected: &str, actual: &str) -> SubmissionVerdict`

Called from `map_outcome`, only on a `Completed` outcome with
`expected_output.is_some()`. Normalizes both strings (trim trailing
whitespace per line, strip trailing blank lines) via a private `normalize`
helper, then does an **exact** match — deliberately simple ("basic"
grading, not a pluggable checker system). Returns `Accepted` or
`WrongAnswer`, never anything else; there is no partial-credit concept
anywhere in this system.

### `codexec_exec_engine::image::ensure_present(cache_root: &Path, image_ref: &str) -> Result<PathBuf, EngineError>`

Called from `ensure_image_cached`. Purely a check — "is this image already
unpacked and its completion marker present?" — it **never pulls**. Returns
the rootfs path on success, `EngineError::ImageNotFound` otherwise. This
function is also what the engine itself calls per-submission (in
`codexec-exec-engine`, not this crate) to resolve an image ref to a rootfs
— meaning a plugin whose image was never successfully pulled by *any*
worker will fail every submission with the same `ImageNotFound` error the
engine surfaces, not just skip the registry's own background-pull attempt.

### `codexec_exec_engine::image::pull_and_unpack(cache_root: &Path, image_ref: &str, source: ImageSource, force: bool) -> Result<PullOutcome, EngineError>`

Called from `ensure_image_cached` with `source: ImageSource::Registry,
force: false`. Idempotent by construction: if a completion marker for this
exact `image_ref` already exists and `force` is `false`, it's a no-op that
just returns the existing rootfs path (`PullOutcome { pulled: false }`) —
this is precisely the property the worker relies on to call this function
opportunistically on *every* registry refresh without ever re-downloading
an already-cached image. When it does need to actually pull, it shells out
to `skopeo` (into an OCI layout dir) then `umoci` (unpacking that into a
plain rootfs bundle dir), clearing any stale layout/bundle directories
first so a partial prior attempt can never merge with a fresh one.
