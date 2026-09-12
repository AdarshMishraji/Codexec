mod admin_auth;
mod api_key_auth;
mod api_keys;
mod config;
mod error;
mod languages;
mod plugin_templates;
mod state;
mod stats;
mod submissions;

use axum::middleware;
use axum::response::Html;
use axum::routing::{delete, get, post};
use axum::Router;
use config::ApiConfig;
use sqlx::postgres::PgPoolOptions;
use state::AppState;
use std::sync::Arc;
use tower_http::trace::TraceLayer;

const DASHBOARD_HTML: &str = include_str!("../assets/dashboard.html");
const ADMIN_HTML: &str = include_str!("../assets/admin.html");

const SUBMISSIONS_STREAM: &str = "SUBMISSIONS";

async fn ensure_stream(js: &async_nats::jetstream::Context) -> anyhow::Result<()> {
    js.get_or_create_stream(async_nats::jetstream::stream::Config {
        name: SUBMISSIONS_STREAM.to_string(),
        subjects: vec!["codexec.submissions.>".to_string()],
        retention: async_nats::jetstream::stream::RetentionPolicy::WorkQueue,
        storage: async_nats::jetstream::stream::StorageType::File,
        ..Default::default()
    })
    .await?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt::init();

    let config = ApiConfig::from_env()?;
    let pool = PgPoolOptions::new()
        .max_connections(config.common.db_pool_size)
        .connect(&config.common.database_url)
        .await?;

    if config.run_migrations_on_startup {
        sqlx::migrate!("../../migrations").run(&pool).await?;
    }

    let nats = async_nats::connect(&config.common.nats_url).await?;
    let jetstream = async_nats::jetstream::new(nats.clone());
    ensure_stream(&jetstream).await?;

    let state = AppState { pool, nats, jetstream, config: Arc::new(config) };

    let admin_routes = Router::new()
        .route("/languages", get(languages::list_admin).post(languages::register))
        .route("/languages/:slug", delete(languages::remove))
        .route("/languages/:slug/activate", post(languages::activate))
        .route("/languages/:slug/deactivate", post(languages::deactivate))
        .route("/api-keys", get(api_keys::list).post(api_keys::create))
        .route("/api-keys/:id", delete(api_keys::remove))
        .route("/plugin-templates", get(plugin_templates::list))
        .route("/plugin-templates/:slug", get(plugin_templates::get_one))
        .route_layer(middleware::from_fn_with_state(state.clone(), admin_auth::require_admin_token));

    let submission_routes = Router::new()
        .route("/submissions", post(submissions::create_submission))
        .route("/submissions/:id", get(submissions::get_submission))
        .route_layer(middleware::from_fn_with_state(state.clone(), api_key_auth::require_api_key));

    let app = Router::new()
        .route("/", get(|| async { Html(DASHBOARD_HTML) }))
        .route("/admin", get(|| async { Html(ADMIN_HTML) }))
        .route("/stats", get(stats::get_stats))
        .route("/languages", get(languages::list_public))
        .merge(submission_routes)
        .nest("/admin", admin_routes)
        .layer(TraceLayer::new_for_http())
        .with_state(state.clone());

    let listener = tokio::net::TcpListener::bind(&state.config.bind_addr).await?;
    tracing::info!("codexec-api listening on {}", state.config.bind_addr);
    axum::serve(listener, app).await?;

    Ok(())
}
