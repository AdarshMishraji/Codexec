//! The interface between the worker's queue-consumer loop and whatever
//! actually runs submitted code (a real containerd-backed engine, or a
//! `tokio::process::Command`-based mock for local dev). Deliberately thin:
//! no containerd/NATS/DB dependencies, so both sides can compile against
//! it independently.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ExecutionRequest {
    /// == submission_id. Unique per execution attempt; the engine derives
    /// its container id / workspace dir name from this.
    pub run_id: Uuid,
    /// OCI image reference, already pulled+unpacked into containerd at
    /// plugin-registration time.
    pub image_ref: String,
    /// argv, not a shell string — no template substitution, no injection risk.
    pub compile_cmd: Option<Vec<String>>,
    pub run_cmd: Vec<String>,
    pub source_filename: String,
    pub source_code: String,
    pub stdin: String,
    /// User-facing limit; engine enforces by polling cgroup cpu.stat and killing.
    pub cpu_time_limit_ms: u64,
    /// User-facing limit; engine enforces directly via cgroup cpu.quota/period.
    pub cpu_limit_cores: f64,
    /// User-facing limit; engine enforces via cgroup memory.max (kernel-enforced).
    pub memory_limit_kb: u64,
    /// From the plugin manifest, not user-facing.
    pub compile_time_limit_ms: u64,
    /// Derived server-side safety-net ceiling, not user-facing.
    pub wall_time_limit_ms: u64,
    pub max_output_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionOutcome {
    Completed {
        exit_code: i32,
        stdout: String,
        stdout_truncated: bool,
        stderr: String,
        stderr_truncated: bool,
        cpu_time_used_ms: u64,
        memory_used_kb: u64,
        wall_time_ms: u64,
    },
    CompileError {
        stderr: String,
        stderr_truncated: bool,
        exit_code: i32,
        cpu_time_used_ms: u64,
        wall_time_ms: u64,
    },
    TimeLimitExceeded {
        stdout: String,
        stderr: String,
        cpu_time_used_ms: u64,
        wall_time_ms: u64,
    },
    MemoryLimitExceeded {
        stdout: String,
        stderr: String,
        cpu_time_used_ms: u64,
        memory_used_kb: u64,
    },
    InternalError {
        message: String,
        /// Tells the queue layer whether redelivery should retry this or
        /// treat it as terminal (e.g. a missing image won't fix itself).
        retryable: bool,
    },
}

#[async_trait]
pub trait ExecutionEngine: Send + Sync {
    /// Infallible by design: `InternalError` IS the error channel, so
    /// callers never need a raw `?`-propagated error for a single
    /// submission — every path, including a containerd outage, yields a
    /// value that can be written back and acked/retried through the queue.
    async fn execute(&self, req: ExecutionRequest) -> ExecutionOutcome;
}
