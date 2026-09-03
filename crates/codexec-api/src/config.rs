use codexec_common::config::{require_var, var_or, CommonConfig, ConfigError};

#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub common: CommonConfig,
    pub bind_addr: String,
    pub admin_api_token: String,
    pub max_source_code_bytes: usize,
    pub platform_max_cpu_time_limit_ms: i32,
    pub platform_max_cpu_limit_cores: f64,
    pub platform_max_memory_limit_kb: i64,
    pub wall_time_multiplier: u64,
    pub wall_time_fixed_overhead_ms: u64,
    pub platform_max_wall_time_ms: u64,
    pub run_migrations_on_startup: bool,
}

impl ApiConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            common: CommonConfig::from_env()?,
            bind_addr: var_or("API_BIND_ADDR", "0.0.0.0:8080".to_string())?,
            admin_api_token: require_var("ADMIN_API_TOKEN")?,
            max_source_code_bytes: var_or("MAX_SOURCE_CODE_BYTES", 65536usize)?,
            platform_max_cpu_time_limit_ms: var_or("PLATFORM_MAX_CPU_TIME_LIMIT_MS", 60_000i32)?,
            platform_max_cpu_limit_cores: var_or("PLATFORM_MAX_CPU_LIMIT_CORES", 8.0f64)?,
            platform_max_memory_limit_kb: var_or("PLATFORM_MAX_MEMORY_LIMIT_KB", 83_886_080i64)?,
            wall_time_multiplier: var_or("WALL_TIME_MULTIPLIER", 3u64)?,
            wall_time_fixed_overhead_ms: var_or("WALL_TIME_FIXED_OVERHEAD_MS", 2_000u64)?,
            platform_max_wall_time_ms: var_or("PLATFORM_MAX_WALL_TIME_MS", 120_000u64)?,
            run_migrations_on_startup: var_or("RUN_MIGRATIONS_ON_STARTUP", true)?,
        })
    }
}
