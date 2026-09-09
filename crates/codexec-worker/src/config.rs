use codexec_common::config::{var_or, CommonConfig, ConfigError};

#[derive(Debug, Clone)]
pub struct WorkerConfig {
    pub common: CommonConfig,
    pub worker_concurrency: usize,
    pub consumer_name: String,
    pub consumer_filter_subject: String,
    pub nats_ack_wait_secs: u64,
    pub nats_max_deliver: i64,
    pub image_cache_root: String,
    pub runc_root: String,
    pub workspace_root: String,
    pub cgroup_root: String,
    pub engine_total_cpu_cores: f64,
    pub engine_total_memory_mb: u64,
}

impl WorkerConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            common: CommonConfig::from_env()?,
            worker_concurrency: var_or("WORKER_CONCURRENCY", 4usize)?,
            consumer_name: var_or("WORKER_CONSUMER_NAME", "workers-generic".to_string())?,
            consumer_filter_subject: var_or(
                "WORKER_CONSUMER_FILTER_SUBJECT",
                "codexec.submissions.>".to_string(),
            )?,
            nats_ack_wait_secs: var_or("NATS_ACK_WAIT_SECS", 120u64)?,
            nats_max_deliver: var_or("NATS_MAX_DELIVER", 5i64)?,
            image_cache_root: var_or("IMAGE_CACHE_ROOT", "/var/lib/codexec/images".to_string())?,
            runc_root: var_or("RUNC_ROOT", "/run/codexec/runc".to_string())?,
            workspace_root: var_or("WORKSPACE_ROOT", "/var/lib/codexec/workspaces".to_string())?,
            cgroup_root: var_or("CGROUP_ROOT", "/sys/fs/cgroup".to_string())?,
            engine_total_cpu_cores: var_or("ENGINE_TOTAL_CPU_CORES", 4.0f64)?,
            engine_total_memory_mb: var_or("ENGINE_TOTAL_MEMORY_MB", 4096u64)?,
        })
    }
}
