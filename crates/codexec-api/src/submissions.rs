use crate::api_key_auth::AuthenticatedApiKey;
use crate::error::ApiError;
use crate::state::AppState;
use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use codexec_common::models::{Language, Submission};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct SubmitRequest {
    pub language: String,
    pub source_code: String,
    #[serde(default)]
    pub stdin: String,
    #[serde(default)]
    pub expected_output: Option<String>,
    pub cpu_time_limit_ms: Option<i32>,
    pub cpu_limit_cores: Option<f64>,
    pub memory_limit_kb: Option<i64>,
}

#[derive(Serialize)]
pub struct SubmitResponse {
    pub id: Uuid,
    pub status: &'static str,
    pub submitted_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct SubmissionResponse {
    pub id: Uuid,
    pub language: String,
    pub status: codexec_common::models::SubmissionStatus,
    pub verdict: Option<codexec_common::models::SubmissionVerdict>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub compile_output: Option<String>,
    pub exit_code: Option<i32>,
    pub cpu_time_limit_ms: i32,
    pub cpu_limit_cores: f64,
    pub memory_limit_kb: i64,
    pub cpu_time_used_ms: Option<f64>,
    pub memory_used_kb: Option<i64>,
    pub limit_exceeded: Option<codexec_common::models::LimitExceeded>,
    pub error_message: Option<String>,
    pub submitted_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

impl From<Submission> for SubmissionResponse {
    fn from(s: Submission) -> Self {
        Self {
            id: s.id,
            language: s.language_slug,
            limit_exceeded: s.status.limit_exceeded(),
            status: s.status,
            verdict: s.verdict,
            stdout: s.stdout,
            stderr: s.stderr,
            compile_output: s.compile_output,
            exit_code: s.exit_code,
            cpu_time_limit_ms: s.cpu_time_limit_ms,
            cpu_limit_cores: s.cpu_limit_cores,
            memory_limit_kb: s.memory_limit_kb,
            cpu_time_used_ms: s.cpu_time_used_ms,
            memory_used_kb: s.memory_used_kb,
            error_message: s.error_message,
            submitted_at: s.submitted_at,
            started_at: s.started_at,
            finished_at: s.finished_at,
        }
    }
}

fn subject_for(language_slug: &str) -> String {
    format!("codexec.submissions.{language_slug}")
}

pub async fn create_submission(
    State(state): State<AppState>,
    Extension(AuthenticatedApiKey(api_key_id)): Extension<AuthenticatedApiKey>,
    Json(req): Json<SubmitRequest>,
) -> Result<(StatusCode, Json<SubmitResponse>), ApiError> {
    let language: Option<Language> =
        sqlx::query_as("SELECT * FROM languages WHERE slug = $1 AND is_active")
            .bind(&req.language)
            .fetch_optional(&state.pool)
            .await?;
    let language = language.ok_or_else(|| ApiError::unknown_language(&req.language))?;

    let effective_max_cpu_time_ms =
        language.max_cpu_time_limit_ms.min(state.config.platform_max_cpu_time_limit_ms);
    let effective_max_cpu_cores = language.max_cpu_limit_cores.min(state.config.platform_max_cpu_limit_cores);
    let effective_max_memory_kb = language.max_memory_limit_kb.min(state.config.platform_max_memory_limit_kb);

    let cpu_time_limit_ms = req.cpu_time_limit_ms.unwrap_or(language.default_cpu_time_limit_ms);
    if cpu_time_limit_ms <= 0 || cpu_time_limit_ms > effective_max_cpu_time_ms {
        return Err(ApiError::invalid(
            "invalid_cpu_time_limit",
            format!("cpu_time_limit_ms must be in (0, {effective_max_cpu_time_ms}]"),
        ));
    }

    let cpu_limit_cores = req.cpu_limit_cores.unwrap_or(language.default_cpu_limit_cores);
    if cpu_limit_cores <= 0.0 || cpu_limit_cores > effective_max_cpu_cores {
        return Err(ApiError::invalid(
            "invalid_cpu_limit_cores",
            format!("cpu_limit_cores must be in (0, {effective_max_cpu_cores}]"),
        ));
    }

    let memory_limit_kb = req.memory_limit_kb.unwrap_or(language.default_memory_limit_kb);
    if memory_limit_kb <= 0 || memory_limit_kb > effective_max_memory_kb {
        return Err(ApiError::invalid(
            "invalid_memory_limit",
            format!("memory_limit_kb must be in (0, {effective_max_memory_kb}]"),
        ));
    }

    if req.source_code.is_empty() || req.source_code.len() > state.config.max_source_code_bytes {
        return Err(ApiError::invalid("source_too_large", "source_code is empty or exceeds the size limit"));
    }
    if let Some(expected) = &req.expected_output {
        if expected.len() > state.config.max_source_code_bytes {
            return Err(ApiError::invalid("source_too_large", "expected_output exceeds the size limit"));
        }
    }

    let id = Uuid::new_v4();
    let mut tx = state.pool.begin().await?;

    let submitted_at: DateTime<Utc> = sqlx::query_scalar(
        r#"
        INSERT INTO submissions (
            id, language_id, language_slug, source_code, stdin, expected_output,
            cpu_time_limit_ms, cpu_limit_cores, memory_limit_kb, status, api_key_id
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'queued', $10)
        RETURNING submitted_at
        "#,
    )
    .bind(id)
    .bind(language.id)
    .bind(&language.slug)
    .bind(&req.source_code)
    .bind(&req.stdin)
    .bind(&req.expected_output)
    .bind(cpu_time_limit_ms)
    .bind(cpu_limit_cores)
    .bind(memory_limit_kb)
    .bind(api_key_id)
    .fetch_one(&mut *tx)
    .await?;

    let payload = serde_json::json!({
        "submission_id": id,
        "language_slug": language.slug,
        "enqueued_at": submitted_at,
    });
    let publish_ok = match state.jetstream.publish(subject_for(&language.slug), payload.to_string().into()).await {
        Ok(ack_future) => ack_future.await.is_ok(),
        Err(_) => false,
    };
    if !publish_ok {
        tx.rollback().await.ok();
        return Err(ApiError::queue_unavailable());
    }

    tx.commit().await?;

    Ok((StatusCode::ACCEPTED, Json(SubmitResponse { id, status: "queued", submitted_at })))
}

pub async fn get_submission(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<SubmissionResponse>, ApiError> {
    let submission: Option<Submission> =
        sqlx::query_as("SELECT * FROM submissions WHERE id = $1").bind(id).fetch_optional(&state.pool).await?;
    let submission = submission.ok_or_else(|| ApiError::not_found("no such submission"))?;
    Ok(Json(submission.into()))
}
