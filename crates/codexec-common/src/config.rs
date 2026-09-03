use std::env;

#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
    #[error("missing required environment variable: {0}")]
    MissingVar(String),
    #[error("invalid value for environment variable {name}: {value}")]
    InvalidValue { name: String, value: String },
}

pub fn require_var(name: &str) -> Result<String, ConfigError> {
    env::var(name).map_err(|_| ConfigError::MissingVar(name.to_string()))
}

pub fn var_or<T: std::str::FromStr>(name: &str, default: T) -> Result<T, ConfigError> {
    match env::var(name) {
        Ok(v) => v
            .parse()
            .map_err(|_| ConfigError::InvalidValue { name: name.to_string(), value: v }),
        Err(_) => Ok(default),
    }
}

/// Config shared by every binary: the DB connection and log filter.
#[derive(Debug, Clone)]
pub struct CommonConfig {
    pub database_url: String,
    pub nats_url: String,
    pub db_pool_size: u32,
}

impl CommonConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            database_url: require_var("DATABASE_URL")?,
            nats_url: var_or("NATS_URL", "nats://127.0.0.1:4222".to_string())?,
            db_pool_size: var_or("DB_POOL_SIZE", 10u32)?,
        })
    }
}
