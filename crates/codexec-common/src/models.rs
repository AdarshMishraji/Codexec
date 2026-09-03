use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(sqlx::Type, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[sqlx(type_name = "submission_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SubmissionStatus {
    Queued,
    Processing,
    Completed,
    CompileError,
    RuntimeError,
    TimeLimitExceeded,
    MemoryLimitExceeded,
    InternalError,
}

#[derive(sqlx::Type, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[sqlx(type_name = "submission_verdict", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SubmissionVerdict {
    Accepted,
    WrongAnswer,
}

/// Which requested limit was breached, if any. Not a DB column itself —
/// derived from `status` for the API response (`limit_exceeded` field).
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LimitExceeded {
    CpuTime,
    Memory,
}

impl SubmissionStatus {
    pub fn limit_exceeded(&self) -> Option<LimitExceeded> {
        match self {
            SubmissionStatus::TimeLimitExceeded => Some(LimitExceeded::CpuTime),
            SubmissionStatus::MemoryLimitExceeded => Some(LimitExceeded::Memory),
            _ => None,
        }
    }
}

#[derive(sqlx::FromRow, Serialize, Deserialize, Debug, Clone)]
pub struct Language {
    pub id: Uuid,
    pub slug: String,
    pub display_name: String,
    pub version: String,
    pub image_ref: String,
    pub compile_cmd: Option<serde_json::Value>, // argv array, or NULL for interpreted languages
    pub run_cmd: serde_json::Value,             // argv array
    pub source_filename: String,
    pub compile_time_limit_ms: i32,
    pub default_cpu_time_limit_ms: i32,
    pub default_cpu_limit_cores: f64,
    pub default_memory_limit_kb: i64,
    pub max_cpu_time_limit_ms: i32,
    pub max_cpu_limit_cores: f64,
    pub max_memory_limit_kb: i64,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow, Serialize, Deserialize, Debug, Clone)]
pub struct Submission {
    pub id: Uuid,
    pub language_id: Uuid,
    pub language_slug: String,
    pub source_code: String,
    pub stdin: String,
    pub expected_output: Option<String>,
    pub cpu_time_limit_ms: i32,
    pub cpu_limit_cores: f64,
    pub memory_limit_kb: i64,
    pub status: SubmissionStatus,
    pub verdict: Option<SubmissionVerdict>,

    pub exit_code: Option<i32>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub compile_output: Option<String>,
    pub cpu_time_used_ms: Option<f64>,
    pub memory_used_kb: Option<i64>,
    pub error_message: Option<String>,

    pub worker_id: Option<String>,
    pub lease_expires_at: Option<DateTime<Utc>>,

    pub submitted_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

/// Parsed `argv` from a `languages.compile_cmd` / `run_cmd` JSONB column.
pub fn parse_argv(value: &serde_json::Value) -> Vec<String> {
    value
        .as_array()
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}
