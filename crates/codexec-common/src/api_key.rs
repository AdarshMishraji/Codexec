use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

/// Full DB row, including `key_hash` — never serialize this directly to a
/// client; API layers should map it into a response DTO that omits the
/// hash (see `codexec-api`'s `api_keys::PublicApiKey`).
#[derive(sqlx::FromRow, Debug, Clone)]
pub struct ApiKeyRow {
    pub id: Uuid,
    pub label: String,
    pub key_prefix: String,
    pub key_hash: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

/// Generates a fresh raw API key: a `cxk_` prefix (so a leaked key is
/// recognizable at a glance) followed by a v4 UUID's 32 hex digits (122
/// bits of CSPRNG randomness — the same source used for submission/run
/// ids elsewhere in this codebase, so no extra RNG dependency is needed).
pub fn generate_raw_key() -> String {
    format!("cxk_{}", Uuid::new_v4().simple())
}

/// SHA-256 hex digest of a raw key. Keys are high-entropy opaque tokens,
/// not user-chosen passwords, so a plain fast hash is the right tool here
/// (no need for a slow password-hashing KDF like argon2/bcrypt, which
/// defends against brute-forcing low-entropy secrets — irrelevant when the
/// secret itself already has 122 bits of randomness).
pub fn hash_key(raw: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// The prefix shown in admin UI listings so a key can be identified again
/// without ever re-displaying (or re-deriving) the full secret.
pub fn key_prefix(raw: &str) -> String {
    raw.chars().take(12).collect()
}

/// Creates a new key and returns both the DB row and the one-time raw
/// key — the raw value is never stored or retrievable again after this
/// call returns, only its hash is.
pub async fn create_api_key(pool: &PgPool, label: &str) -> Result<(ApiKeyRow, String), sqlx::Error> {
    let raw = generate_raw_key();
    let prefix = key_prefix(&raw);
    let hash = hash_key(&raw);

    let row = sqlx::query_as::<_, ApiKeyRow>(
        r#"
        INSERT INTO api_keys (id, label, key_prefix, key_hash, is_active, created_at)
        VALUES ($1, $2, $3, $4, TRUE, now())
        RETURNING *
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(label)
    .bind(&prefix)
    .bind(&hash)
    .fetch_one(pool)
    .await?;

    Ok((row, raw))
}

pub async fn list_api_keys(pool: &PgPool) -> Result<Vec<ApiKeyRow>, sqlx::Error> {
    sqlx::query_as::<_, ApiKeyRow>("SELECT * FROM api_keys ORDER BY created_at DESC").fetch_all(pool).await
}

/// Hard-deletes the key row (submissions that reference it keep their
/// row via `ON DELETE SET NULL`, just losing the specific attribution).
/// Returns `false` if no such key existed.
pub async fn delete_api_key(pool: &PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM api_keys WHERE id = $1").bind(id).execute(pool).await?;
    Ok(result.rows_affected() > 0)
}

/// Verifies a raw key presented by a client: active and matching some
/// stored hash. Also best-effort bumps `last_used_at` on success. Returns
/// the key's id for attribution (e.g. `submissions.api_key_id`).
pub async fn verify_api_key(pool: &PgPool, raw: &str) -> Result<Option<Uuid>, sqlx::Error> {
    let hash = hash_key(raw);
    let id: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM api_keys WHERE key_hash = $1 AND is_active").bind(&hash).fetch_optional(pool).await?;

    if let Some(id) = id {
        // Best-effort: a failed timestamp bump shouldn't fail authentication.
        let _ = sqlx::query("UPDATE api_keys SET last_used_at = now() WHERE id = $1").bind(id).execute(pool).await;
    }

    Ok(id)
}
