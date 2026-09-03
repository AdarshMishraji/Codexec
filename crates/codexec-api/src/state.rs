use crate::config::ApiConfig;
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub nats: async_nats::Client,
    pub jetstream: async_nats::jetstream::Context,
    pub config: Arc<ApiConfig>,
}
