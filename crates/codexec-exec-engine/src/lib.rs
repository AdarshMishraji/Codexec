//! `runc`-backed implementation of `codexec_exec_contract::ExecutionEngine`.
//! No daemon: images are pulled+unpacked to a plain rootfs directory ahead
//! of time (see `image.rs`, driven by `codexec-plugin-cli` via `skopeo` +
//! `umoci`), and each submission is a direct `runc` subprocess invocation
//! against that shared, read-only rootfs — no snapshotter, no gRPC, no
//! background process to keep alive. Needs a real cgroup v2 Linux host.

mod admission;
mod cgroup;
mod classify;
mod error;
pub mod image;
mod lifecycle;
mod spec;
mod workspace;
mod wrapper;

pub use error::EngineError;
pub use lifecycle::RuncExecutionEngine;

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Where unpacked image rootfs directories are cached, shared
    /// read-only across every submission for a given image. Populated by
    /// `codexec-plugin-cli` (see `image::pull_and_unpack`); the engine
    /// itself only ever reads from here, never pulls on demand.
    pub image_cache_root: PathBuf,
    /// `runc`'s `--root` state directory (tracks created/running
    /// containers) - dedicated to codexec so it can't collide with any
    /// other runc usage on the host (e.g. Docker's own).
    pub runc_root: PathBuf,
    pub workspace_root: PathBuf,
    pub cgroup_root: PathBuf,
    pub default_compile_time_limit_ms: u64,
    pub kill_grace_period_ms: u64,
    pub default_max_output_bytes: u64,
    pub cpu_poll_interval_ms: u64,
    pub total_cpu_cores: f64,
    pub total_memory_bytes: u64,
}
