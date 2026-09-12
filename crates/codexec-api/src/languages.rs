use crate::error::ApiError;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use codexec_common::models::Language;
use codexec_common::registry::{self, PluginManifest, RegistryError};
use serde::Serialize;

#[derive(Serialize)]
pub struct PublicLanguage {
    pub slug: String,
    pub display_name: String,
    pub version: String,
    pub default_cpu_time_limit_ms: i32,
    pub default_cpu_limit_cores: f64,
    pub default_memory_limit_kb: i64,
    pub max_cpu_time_limit_ms: i32,
    pub max_cpu_limit_cores: f64,
    pub max_memory_limit_kb: i64,
}

impl From<Language> for PublicLanguage {
    fn from(l: Language) -> Self {
        Self {
            slug: l.slug,
            display_name: l.display_name,
            version: l.version,
            default_cpu_time_limit_ms: l.default_cpu_time_limit_ms,
            default_cpu_limit_cores: l.default_cpu_limit_cores,
            default_memory_limit_kb: l.default_memory_limit_kb,
            max_cpu_time_limit_ms: l.max_cpu_time_limit_ms,
            max_cpu_limit_cores: l.max_cpu_limit_cores,
            max_memory_limit_kb: l.max_memory_limit_kb,
        }
    }
}

/// Public listing: active languages only, no image_ref/commands exposed.
pub async fn list_public(State(state): State<AppState>) -> Result<Json<Vec<PublicLanguage>>, ApiError> {
    let languages: Vec<Language> =
        sqlx::query_as("SELECT * FROM languages WHERE is_active ORDER BY slug").fetch_all(&state.pool).await?;
    Ok(Json(languages.into_iter().map(Into::into).collect()))
}

/// Admin listing: full detail including inactive languages and image_ref/commands.
pub async fn list_admin(State(state): State<AppState>) -> Result<Json<Vec<Language>>, ApiError> {
    let languages: Vec<Language> = sqlx::query_as("SELECT * FROM languages ORDER BY slug").fetch_all(&state.pool).await?;
    Ok(Json(languages))
}

pub async fn register(
    State(state): State<AppState>,
    Json(manifest): Json<PluginManifest>,
) -> Result<Json<Language>, ApiError> {
    let language = registry::register_language(&state.pool, Some(&state.nats), &manifest)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(language))
}

pub async fn activate(State(state): State<AppState>, Path(slug): Path<String>) -> Result<Json<Language>, ApiError> {
    registry::set_active(&state.pool, Some(&state.nats), &slug, true)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("no such language"))
}

pub async fn deactivate(State(state): State<AppState>, Path(slug): Path<String>) -> Result<Json<Language>, ApiError> {
    registry::set_active(&state.pool, Some(&state.nats), &slug, false)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("no such language"))
}

pub async fn remove(State(state): State<AppState>, Path(slug): Path<String>) -> Result<StatusCode, ApiError> {
    let deleted = registry::delete_language(&state.pool, Some(&state.nats), &slug).await.map_err(|e| match e {
        RegistryError::InUse => ApiError::conflict(
            "this plugin has existing submissions and can't be deleted - deactivate it instead",
        ),
        other => ApiError::internal(other.to_string()),
    })?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("no such language"))
    }
}
