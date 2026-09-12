use crate::error::ApiError;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use codexec_common::api_key::{self, ApiKeyRow};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct CreateApiKeyRequest {
    pub label: String,
}

/// Never includes `key_hash` — the raw key is shown exactly once, in
/// `CreateApiKeyResponse`, at creation time.
#[derive(Serialize)]
pub struct PublicApiKey {
    pub id: Uuid,
    pub label: String,
    pub key_prefix: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

impl From<ApiKeyRow> for PublicApiKey {
    fn from(r: ApiKeyRow) -> Self {
        Self {
            id: r.id,
            label: r.label,
            key_prefix: r.key_prefix,
            is_active: r.is_active,
            created_at: r.created_at,
            last_used_at: r.last_used_at,
            revoked_at: r.revoked_at,
        }
    }
}

#[derive(Serialize)]
pub struct CreateApiKeyResponse {
    #[serde(flatten)]
    pub key: PublicApiKey,
    /// Shown once. Not recoverable afterward — only its hash is stored.
    pub api_key: String,
}

pub async fn create(
    State(state): State<AppState>,
    Json(req): Json<CreateApiKeyRequest>,
) -> Result<(StatusCode, Json<CreateApiKeyResponse>), ApiError> {
    let label = req.label.trim();
    if label.is_empty() {
        return Err(ApiError::invalid("invalid_label", "label must not be empty"));
    }

    let (row, raw_key) = api_key::create_api_key(&state.pool, label).await?;
    Ok((StatusCode::CREATED, Json(CreateApiKeyResponse { key: row.into(), api_key: raw_key })))
}

pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<PublicApiKey>>, ApiError> {
    let rows = api_key::list_api_keys(&state.pool).await?;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

pub async fn remove(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<StatusCode, ApiError> {
    let deleted = api_key::delete_api_key(&state.pool, id).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("no such API key"))
    }
}
