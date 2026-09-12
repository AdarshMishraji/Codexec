use crate::error::ApiError;
use crate::state::AppState;
use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;
use uuid::Uuid;

/// Injected into request extensions on successful auth so handlers can
/// attribute what they do (e.g. a new submission) to the calling key.
#[derive(Clone, Copy)]
pub struct AuthenticatedApiKey(pub Uuid);

pub async fn require_api_key(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let provided = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::to_string);

    let Some(raw_key) = provided else {
        return Err(ApiError::invalid_api_key());
    };

    let key_id = codexec_common::api_key::verify_api_key(&state.pool, &raw_key)
        .await?
        .ok_or_else(ApiError::invalid_api_key)?;

    req.extensions_mut().insert(AuthenticatedApiKey(key_id));
    Ok(next.run(req).await)
}
