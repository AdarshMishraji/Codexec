use crate::error::ApiError;
use crate::state::AppState;
use axum::extract::State;
use axum::Json;
use chrono::{DateTime, NaiveDate, Utc};
use codexec_common::models::{SubmissionStatus, SubmissionVerdict};
use serde::Serialize;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct LanguageCounts {
    total: i64,
    active: i64,
}

#[derive(sqlx::FromRow)]
struct ApiKeyCounts {
    total: i64,
    active: i64,
}

#[derive(sqlx::FromRow)]
struct SubmissionAggregates {
    total: i64,
    completed: i64,
    last_24h: i64,
    last_7d: i64,
    avg_cpu_ms: Option<f64>,
    avg_mem_kb: Option<f64>,
    p50_cpu_ms: Option<f64>,
    p95_cpu_ms: Option<f64>,
    avg_wall_ms: Option<f64>,
    avg_queue_ms: Option<f64>,
}

#[derive(sqlx::FromRow, Serialize)]
struct StatusCountRow {
    status: SubmissionStatus,
    count: i64,
}

#[derive(sqlx::FromRow, Serialize)]
struct VerdictCountRow {
    verdict: SubmissionVerdict,
    count: i64,
}

#[derive(sqlx::FromRow, Serialize)]
struct LanguageCountRow {
    slug: String,
    display_name: String,
    count: i64,
}

#[derive(sqlx::FromRow, Serialize)]
struct DailyRow {
    day: NaiveDate,
    count: i64,
}

#[derive(sqlx::FromRow, Serialize)]
struct RecentSubmissionRow {
    id: Uuid,
    language_slug: String,
    status: SubmissionStatus,
    verdict: Option<SubmissionVerdict>,
    cpu_time_used_ms: Option<f64>,
    memory_used_kb: Option<i64>,
    submitted_at: DateTime<Utc>,
    finished_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
pub struct StatsResponse {
    total_submissions: i64,
    completed_submissions: i64,
    success_rate_pct: f64,
    submissions_last_24h: i64,
    submissions_last_7d: i64,
    avg_cpu_time_ms: Option<f64>,
    avg_memory_kb: Option<f64>,
    p50_cpu_time_ms: Option<f64>,
    p95_cpu_time_ms: Option<f64>,
    avg_wall_time_ms: Option<f64>,
    avg_queue_wait_ms: Option<f64>,
    languages_total: i64,
    languages_active: i64,
    api_keys_total: i64,
    api_keys_active: i64,
    status_breakdown: Vec<StatusCountRow>,
    verdict_breakdown: Vec<VerdictCountRow>,
    language_breakdown: Vec<LanguageCountRow>,
    daily_trend: Vec<DailyRow>,
    recent_submissions: Vec<RecentSubmissionRow>,
}

/// Public, unauthenticated: every number here is an aggregate or
/// submission metadata (id/language/status/verdict/timestamps/resource
/// usage) — never source code, stdin, or stdout/stderr content, which
/// stay behind an active API key (see `submissions::get_submission`).
pub async fn get_stats(State(state): State<AppState>) -> Result<Json<StatsResponse>, ApiError> {
    let pool = &state.pool;

    let languages = sqlx::query_as::<_, LanguageCounts>(
        "SELECT COUNT(*) AS total, COUNT(*) FILTER (WHERE is_active) AS active FROM languages",
    )
    .fetch_one(pool);

    let api_keys = sqlx::query_as::<_, ApiKeyCounts>(
        "SELECT COUNT(*) AS total, COUNT(*) FILTER (WHERE is_active) AS active FROM api_keys",
    )
    .fetch_one(pool);

    let aggregates = sqlx::query_as::<_, SubmissionAggregates>(
        r#"
        SELECT
            COUNT(*) AS total,
            COUNT(*) FILTER (WHERE status = 'completed') AS completed,
            COUNT(*) FILTER (WHERE submitted_at > now() - interval '24 hours') AS last_24h,
            COUNT(*) FILTER (WHERE submitted_at > now() - interval '7 days') AS last_7d,
            AVG(cpu_time_used_ms) AS avg_cpu_ms,
            AVG(memory_used_kb)::float8 AS avg_mem_kb,
            percentile_cont(0.5) WITHIN GROUP (ORDER BY cpu_time_used_ms) AS p50_cpu_ms,
            percentile_cont(0.95) WITHIN GROUP (ORDER BY cpu_time_used_ms) AS p95_cpu_ms,
            AVG(EXTRACT(EPOCH FROM (finished_at - started_at)) * 1000)::float8 AS avg_wall_ms,
            AVG(EXTRACT(EPOCH FROM (started_at - submitted_at)) * 1000)::float8 AS avg_queue_ms
        FROM submissions
        "#,
    )
    .fetch_one(pool);

    let status_breakdown = sqlx::query_as::<_, StatusCountRow>(
        "SELECT status, COUNT(*) AS count FROM submissions GROUP BY status ORDER BY count DESC",
    )
    .fetch_all(pool);

    let verdict_breakdown = sqlx::query_as::<_, VerdictCountRow>(
        "SELECT verdict, COUNT(*) AS count FROM submissions WHERE verdict IS NOT NULL GROUP BY verdict ORDER BY count DESC",
    )
    .fetch_all(pool);

    let language_breakdown = sqlx::query_as::<_, LanguageCountRow>(
        r#"
        SELECT s.language_slug AS slug, l.display_name AS display_name, COUNT(*) AS count
        FROM submissions s
        JOIN languages l ON l.id = s.language_id
        GROUP BY s.language_slug, l.display_name
        ORDER BY count DESC
        "#,
    )
    .fetch_all(pool);

    let daily_trend = sqlx::query_as::<_, DailyRow>(
        r#"
        SELECT d::date AS day, COUNT(s.id) AS count
        FROM generate_series((now()::date - interval '13 days'), now()::date, interval '1 day') AS d
        LEFT JOIN submissions s ON date_trunc('day', s.submitted_at) = d
        GROUP BY d
        ORDER BY d
        "#,
    )
    .fetch_all(pool);

    let recent_submissions = sqlx::query_as::<_, RecentSubmissionRow>(
        r#"
        SELECT id, language_slug, status, verdict, cpu_time_used_ms, memory_used_kb, submitted_at, finished_at
        FROM submissions
        ORDER BY submitted_at DESC
        LIMIT 20
        "#,
    )
    .fetch_all(pool);

    let (languages, api_keys, aggregates, status_breakdown, verdict_breakdown, language_breakdown, daily_trend, recent_submissions) =
        tokio::try_join!(
            languages,
            api_keys,
            aggregates,
            status_breakdown,
            verdict_breakdown,
            language_breakdown,
            daily_trend,
            recent_submissions
        )?;

    let success_rate_pct = if aggregates.total > 0 {
        (aggregates.completed as f64 / aggregates.total as f64) * 100.0
    } else {
        0.0
    };

    Ok(Json(StatsResponse {
        total_submissions: aggregates.total,
        completed_submissions: aggregates.completed,
        success_rate_pct,
        submissions_last_24h: aggregates.last_24h,
        submissions_last_7d: aggregates.last_7d,
        avg_cpu_time_ms: aggregates.avg_cpu_ms,
        avg_memory_kb: aggregates.avg_mem_kb,
        p50_cpu_time_ms: aggregates.p50_cpu_ms,
        p95_cpu_time_ms: aggregates.p95_cpu_ms,
        avg_wall_time_ms: aggregates.avg_wall_ms,
        avg_queue_wait_ms: aggregates.avg_queue_ms,
        languages_total: languages.total,
        languages_active: languages.active,
        api_keys_total: api_keys.total,
        api_keys_active: api_keys.active,
        status_breakdown,
        verdict_breakdown,
        language_breakdown,
        daily_trend,
        recent_submissions,
    }))
}
