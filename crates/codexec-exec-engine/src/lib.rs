//! containerd-backed implementation of `codexec_exec_contract::ExecutionEngine`.
//! Needs a real containerd socket + cgroup v2 host to run — Linux only.
//! Built out in full in `lifecycle.rs`; see the project plan for the
//! per-submission flow (image resolution, OCI spec, wrapper script,
//! cgroup-based limit enforcement and stats).

mod admission;
mod cgroup;
mod classify;
mod error;
mod image;
mod lifecycle;
mod spec;
mod workspace;
mod wrapper;

pub use error::EngineError;
pub use lifecycle::ContainerdExecutionEngine;

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub containerd_socket_path: PathBuf,
    pub namespace: String,
    pub snapshotter: String,
    pub workspace_root: PathBuf,
    pub cgroup_root: PathBuf,
    pub default_compile_time_limit_ms: u64,
    pub kill_grace_period_ms: u64,
    pub default_max_output_bytes: u64,
    pub cpu_poll_interval_ms: u64,
    pub total_cpu_cores: f64,
    pub total_memory_bytes: u64,
}
