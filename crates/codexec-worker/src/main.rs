mod config;
mod registry;

use codexec_common::grading::grade;
use codexec_common::models::{parse_argv, Language, Submission, SubmissionStatus};
use codexec_exec_contract::{ExecutionEngine, ExecutionOutcome, ExecutionRequest};
use config::WorkerConfig;
use futures::StreamExt;
use registry::PluginRegistry;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use uuid::Uuid;

const WALL_TIME_MULTIPLIER: u64 = 3;
const WALL_TIME_FIXED_OVERHEAD_MS: u64 = 2000;
const PLATFORM_MAX_WALL_TIME_MS: u64 = 120_000;
const MAX_OUTPUT_BYTES: u64 = 1_048_576;

async fn build_engine(config: &WorkerConfig) -> Arc<dyn ExecutionEngine> {
    let engine_config = codexec_exec_engine::EngineConfig {
        containerd_socket_path: config.containerd_socket_path.clone().into(),
        namespace: config.containerd_namespace.clone(),
        snapshotter: config.containerd_snapshotter.clone(),
        workspace_root: config.workspace_root.clone().into(),
        cgroup_root: config.cgroup_root.clone().into(),
        default_compile_time_limit_ms: 10_000,
        kill_grace_period_ms: 2_000,
        default_max_output_bytes: MAX_OUTPUT_BYTES,
        cpu_poll_interval_ms: 25,
        total_cpu_cores: config.engine_total_cpu_cores,
        total_memory_bytes: config.engine_total_memory_mb * 1024 * 1024,
    };
    let engine = codexec_exec_engine::ContainerdExecutionEngine::connect(engine_config)
        .await
        .expect("failed to connect to containerd");
    Arc::new(engine)
}

fn build_request(run_id: Uuid, submission: &Submission, language: &Language) -> ExecutionRequest {
    let wall_time_limit_ms = ((submission.cpu_time_limit_ms as u64) * WALL_TIME_MULTIPLIER
        + WALL_TIME_FIXED_OVERHEAD_MS)
        .min(PLATFORM_MAX_WALL_TIME_MS);

    ExecutionRequest {
        run_id,
        image_ref: language.image_ref.clone(),
        compile_cmd: language.compile_cmd.as_ref().map(parse_argv),
        run_cmd: parse_argv(&language.run_cmd),
        source_filename: language.source_filename.clone(),
        source_code: submission.source_code.clone(),
        stdin: submission.stdin.clone(),
        cpu_time_limit_ms: submission.cpu_time_limit_ms as u64,
        cpu_limit_cores: submission.cpu_limit_cores,
        memory_limit_kb: submission.memory_limit_kb as u64,
        compile_time_limit_ms: language.compile_time_limit_ms as u64,
        wall_time_limit_ms,
        max_output_bytes: MAX_OUTPUT_BYTES,
    }
}

/// Maps an outcome + the submission's optional expected_output to the
/// values written back to Postgres. `Completed` with a nonzero exit code
/// becomes `runtime_error`, not `completed` — the engine doesn't
/// distinguish "crashed" from "clean nonzero exit"; that one `if` belongs
/// here, not in the engine.
struct WriteBack {
    status: SubmissionStatus,
    verdict: Option<codexec_common::models::SubmissionVerdict>,
    exit_code: Option<i32>,
    stdout: Option<String>,
    stderr: Option<String>,
    compile_output: Option<String>,
    cpu_time_used_ms: Option<f64>,
    memory_used_kb: Option<i64>,
    error_message: Option<String>,
}

fn map_outcome(outcome: ExecutionOutcome, expected_output: Option<&str>) -> WriteBack {
    match outcome {
        ExecutionOutcome::Completed { exit_code, stdout, stderr, cpu_time_used_ms, memory_used_kb, .. } => {
            let status = if exit_code == 0 { SubmissionStatus::Completed } else { SubmissionStatus::RuntimeError };
            let verdict = if status == SubmissionStatus::Completed {
                expected_output.map(|expected| grade(expected, &stdout))
            } else {
                None
            };
            WriteBack {
                status,
                verdict,
                exit_code: Some(exit_code),
                stdout: Some(stdout),
                stderr: Some(stderr),
                compile_output: None,
                cpu_time_used_ms: Some(cpu_time_used_ms as f64),
                memory_used_kb: Some(memory_used_kb as i64),
                error_message: None,
            }
        }
        ExecutionOutcome::CompileError { stderr, exit_code, cpu_time_used_ms, .. } => WriteBack {
            status: SubmissionStatus::CompileError,
            verdict: None,
            exit_code: Some(exit_code),
            stdout: None,
            stderr: None,
            compile_output: Some(stderr),
            cpu_time_used_ms: Some(cpu_time_used_ms as f64),
            memory_used_kb: None,
            error_message: None,
        },
        ExecutionOutcome::TimeLimitExceeded { stdout, stderr, cpu_time_used_ms, .. } => WriteBack {
            status: SubmissionStatus::TimeLimitExceeded,
            verdict: None,
            exit_code: None,
            stdout: Some(stdout),
            stderr: Some(stderr),
            compile_output: None,
            cpu_time_used_ms: Some(cpu_time_used_ms as f64),
            memory_used_kb: None,
            error_message: None,
        },
        ExecutionOutcome::MemoryLimitExceeded { stdout, stderr, cpu_time_used_ms, memory_used_kb } => WriteBack {
            status: SubmissionStatus::MemoryLimitExceeded,
            verdict: None,
            exit_code: None,
            stdout: Some(stdout),
            stderr: Some(stderr),
            compile_output: None,
            cpu_time_used_ms: Some(cpu_time_used_ms as f64),
            memory_used_kb: Some(memory_used_kb as i64),
            error_message: None,
        },
        ExecutionOutcome::InternalError { message, .. } => WriteBack {
            status: SubmissionStatus::InternalError,
            verdict: None,
            exit_code: None,
            stdout: None,
            stderr: None,
            compile_output: None,
            cpu_time_used_ms: None,
            memory_used_kb: None,
            error_message: Some(message),
        },
    }
}

async fn write_back(pool: &PgPool, id: Uuid, wb: WriteBack) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE submissions SET
            status = $1, verdict = $2, exit_code = $3, stdout = $4, stderr = $5,
            compile_output = $6, cpu_time_used_ms = $7, memory_used_kb = $8,
            error_message = $9, finished_at = now(), updated_at = now()
        WHERE id = $10
        "#,
    )
    .bind(wb.status)
    .bind(wb.verdict)
    .bind(wb.exit_code)
    .bind(wb.stdout)
    .bind(wb.stderr)
    .bind(wb.compile_output)
    .bind(wb.cpu_time_used_ms)
    .bind(wb.memory_used_kb)
    .bind(wb.error_message)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

async fn handle_message(
    msg: async_nats::jetstream::Message,
    pool: PgPool,
    registry: Arc<PluginRegistry>,
    engine: Arc<dyn ExecutionEngine>,
    worker_id: String,
    ack_wait_secs: u64,
) {
    let Ok(payload) = serde_json::from_slice::<serde_json::Value>(&msg.payload) else {
        tracing::error!("poison message: undecodable payload, acking away");
        let _ = msg.ack().await;
        return;
    };
    let Some(submission_id) = payload.get("submission_id").and_then(|v| v.as_str()).and_then(|s| s.parse::<Uuid>().ok())
    else {
        tracing::error!("poison message: missing/invalid submission_id, acking away");
        let _ = msg.ack().await;
        return;
    };

    if let Ok(info) = msg.info() {
        if info.delivered >= 5 {
            let _ = sqlx::query(
                "UPDATE submissions SET status = 'internal_error', error_message = 'exceeded max delivery attempts', updated_at = now() WHERE id = $1 AND status NOT IN ('completed','compile_error','runtime_error','time_limit_exceeded','memory_limit_exceeded','internal_error')",
            )
            .bind(submission_id)
            .execute(&pool)
            .await;
            let _ = msg.ack().await;
            return;
        }
    }

    let claimed: Option<Submission> = sqlx::query_as(
        r#"
        UPDATE submissions
        SET status = 'processing', started_at = now(), worker_id = $1,
            lease_expires_at = now() + ($2 || ' seconds')::interval, updated_at = now()
        WHERE id = $3 AND status = 'queued'
        RETURNING *
        "#,
    )
    .bind(&worker_id)
    .bind(ack_wait_secs.to_string())
    .bind(submission_id)
    .fetch_optional(&pool)
    .await
    .ok()
    .flatten();

    let submission = match claimed {
        Some(s) => s,
        None => {
            let existing: Option<Submission> =
                sqlx::query_as("SELECT * FROM submissions WHERE id = $1").bind(submission_id).fetch_optional(&pool).await.ok().flatten();
            match existing {
                // Row not visible yet on this connection. The API publishes to JetStream (and
                // awaits the publish ack) *before* committing its INSERT transaction, so a
                // worker can legitimately pull and try to claim a submission a few milliseconds
                // before the row is visible to a fresh connection. NAK with a short delay so
                // JetStream redelivers shortly after the commit lands, instead of treating this
                // as a permanent poison message and losing the submission (it would otherwise
                // stay 'queued' forever). Only genuinely-missing rows exhaust max_deliver and
                // get caught by the delivered-count check at the top of this function.
                None => {
                    let _ = msg.ack_with(async_nats::jetstream::AckKind::Nak(Some(Duration::from_millis(250)))).await;
                    return;
                }
                Some(s) if s.status != SubmissionStatus::Queued && s.status != SubmissionStatus::Processing => {
                    let _ = msg.ack().await; // already terminal: ack was lost, work already done
                    return;
                }
                Some(s) if s.status == SubmissionStatus::Processing => {
                    let expired = s.lease_expires_at.map(|t| t <= chrono::Utc::now()).unwrap_or(true);
                    if !expired {
                        let _ = msg.ack_with(async_nats::jetstream::AckKind::Nak(Some(Duration::from_secs(5)))).await;
                        return;
                    }
                    // Lease expired: prior claimant likely crashed. Re-claim.
                    let reclaimed: Option<Submission> = sqlx::query_as(
                        r#"
                        UPDATE submissions
                        SET started_at = now(), worker_id = $1,
                            lease_expires_at = now() + ($2 || ' seconds')::interval, updated_at = now()
                        WHERE id = $3 AND status = 'processing'
                        RETURNING *
                        "#,
                    )
                    .bind(&worker_id)
                    .bind(ack_wait_secs.to_string())
                    .bind(submission_id)
                    .fetch_optional(&pool)
                    .await
                    .ok()
                    .flatten();
                    match reclaimed {
                        Some(s) => s,
                        None => {
                            let _ = msg.ack_with(async_nats::jetstream::AckKind::Nak(Some(Duration::from_millis(250)))).await;
                            return;
                        }
                    }
                }
                // Status is still 'queued': our own claim UPDATE (WHERE status='queued') raced
                // against the same visibility window as the None case above and lost. Retry via
                // redelivery rather than silently dropping the submission.
                Some(_) => {
                    let _ = msg.ack_with(async_nats::jetstream::AckKind::Nak(Some(Duration::from_millis(250)))).await;
                    return;
                }
            }
        }
    };

    // The in-memory cache is a best-effort mirror (updated via the control
    // subject and a periodic full refresh, see registry.rs) — it can lag
    // a fresh activation by up to the control-message delivery latency.
    // Before declaring the language unavailable, fall back to a direct
    // one-off lookup so a language activated moments ago doesn't spuriously
    // fail submissions that race ahead of the cache update.
    let language = match registry.get(&submission.language_slug).await {
        Some(l) => l,
        None => match registry.fetch_fresh(&submission.language_slug).await {
            Some(l) => l,
            None => {
                write_back(
                    &pool,
                    submission.id,
                    WriteBack {
                        status: SubmissionStatus::InternalError,
                        verdict: None,
                        exit_code: None,
                        stdout: None,
                        stderr: None,
                        compile_output: None,
                        cpu_time_used_ms: None,
                        memory_used_kb: None,
                        error_message: Some(format!("language {} no longer active", submission.language_slug)),
                    },
                )
                .await
                .ok();
                let _ = msg.ack().await;
                return;
            }
        },
    };

    let req = build_request(submission.id, &submission, &language);
    let outcome = engine.execute(req).await;
    let wb = map_outcome(outcome, submission.expected_output.as_deref());

    if let Err(e) = write_back(&pool, submission.id, wb).await {
        tracing::error!(%submission_id, error = %e, "failed to write back submission result, not acking (will redeliver)");
        return;
    }

    let _ = msg.ack().await;
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt::init();

    let config = WorkerConfig::from_env()?;
    let pool = PgPoolOptions::new()
        .max_connections(config.common.db_pool_size)
        .connect(&config.common.database_url)
        .await?;

    let nats = async_nats::connect(&config.common.nats_url).await?;
    let jetstream = async_nats::jetstream::new(nats.clone());

    let stream = jetstream
        .get_or_create_stream(async_nats::jetstream::stream::Config {
            name: "SUBMISSIONS".to_string(),
            subjects: vec!["codexec.submissions.>".to_string()],
            retention: async_nats::jetstream::stream::RetentionPolicy::WorkQueue,
            storage: async_nats::jetstream::stream::StorageType::File,
            ..Default::default()
        })
        .await?;

    let consumer = stream
        .get_or_create_consumer(
            &config.consumer_name,
            async_nats::jetstream::consumer::pull::Config {
                durable_name: Some(config.consumer_name.clone()),
                filter_subject: config.consumer_filter_subject.clone(),
                ack_policy: async_nats::jetstream::consumer::AckPolicy::Explicit,
                ack_wait: Duration::from_secs(config.nats_ack_wait_secs),
                max_deliver: config.nats_max_deliver,
                ..Default::default()
            },
        )
        .await?;

    let registry = PluginRegistry::load(pool.clone()).await?;
    registry::spawn_control_subscriber(registry.clone(), nats.clone());
    registry::spawn_periodic_refresh(registry.clone(), Duration::from_secs(60));

    let engine = build_engine(&config).await;
    let semaphore = Arc::new(Semaphore::new(config.worker_concurrency));
    let worker_id = format!("{}-{}", hostname(), Uuid::new_v4());

    tracing::info!(worker_id, "codexec-worker starting");

    let mut messages = consumer.messages().await?;
    while let Some(msg) = messages.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(e) => {
                tracing::error!(error = %e, "error pulling from consumer");
                continue;
            }
        };
        let permit = semaphore.clone().acquire_owned().await?;
        let (pool, registry, engine, worker_id) = (pool.clone(), registry.clone(), engine.clone(), worker_id.clone());
        let ack_wait_secs = config.nats_ack_wait_secs;
        tokio::spawn(async move {
            let _permit = permit;
            handle_message(msg, pool, registry, engine, worker_id, ack_wait_secs).await;
        });
    }

    Ok(())
}

fn hostname() -> String {
    std::env::var("HOSTNAME").unwrap_or_else(|_| "worker".to_string())
}
