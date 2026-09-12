use crate::models::Language;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(thiserror::Error, Debug)]
pub enum RegistryError {
    #[error("manifest parse error: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("plugin has existing submissions and cannot be deleted - deactivate it instead")]
    InUse,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct PluginManifest {
    pub language: LanguageSection,
    pub image: ImageSection,
    pub commands: CommandsSection,
    pub limits: LimitsSection,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct LanguageSection {
    pub slug: String,
    pub display_name: String,
    pub version: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ImageSection {
    pub reference: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct CommandsSection {
    #[serde(default)]
    pub compile_cmd: Vec<String>,
    pub run_cmd: Vec<String>,
    pub source_filename: String,
    pub compile_time_limit_ms: i32,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct LimitsSection {
    pub default_cpu_time_limit_ms: i32,
    pub default_cpu_limit_cores: f64,
    pub default_memory_limit_kb: i64,
    pub max_cpu_time_limit_ms: i32,
    pub max_cpu_limit_cores: f64,
    pub max_memory_limit_kb: i64,
}

impl PluginManifest {
    pub fn from_toml_str(s: &str) -> Result<Self, RegistryError> {
        Ok(toml::from_str(s)?)
    }
}

/// The one code path both `codexec-plugin-cli` and the admin HTTP
/// endpoints call to register/update a language plugin — upsert into
/// Postgres, then notify already-running workers via the control subject
/// so they pick it up without a restart.
pub async fn register_language(
    pool: &PgPool,
    nats: Option<&async_nats::Client>,
    manifest: &PluginManifest,
) -> Result<Language, RegistryError> {
    let compile_cmd = if manifest.commands.compile_cmd.is_empty() {
        None
    } else {
        Some(serde_json::to_value(&manifest.commands.compile_cmd).unwrap())
    };
    let run_cmd = serde_json::to_value(&manifest.commands.run_cmd).unwrap();

    let language = sqlx::query_as::<_, Language>(
        r#"
        INSERT INTO languages (
            id, slug, display_name, version, image_ref, compile_cmd, run_cmd, source_filename,
            compile_time_limit_ms, default_cpu_time_limit_ms, default_cpu_limit_cores,
            default_memory_limit_kb, max_cpu_time_limit_ms, max_cpu_limit_cores, max_memory_limit_kb,
            is_active, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, TRUE, now(), now())
        ON CONFLICT (slug) DO UPDATE SET
            display_name = EXCLUDED.display_name,
            version = EXCLUDED.version,
            image_ref = EXCLUDED.image_ref,
            compile_cmd = EXCLUDED.compile_cmd,
            run_cmd = EXCLUDED.run_cmd,
            source_filename = EXCLUDED.source_filename,
            compile_time_limit_ms = EXCLUDED.compile_time_limit_ms,
            default_cpu_time_limit_ms = EXCLUDED.default_cpu_time_limit_ms,
            default_cpu_limit_cores = EXCLUDED.default_cpu_limit_cores,
            default_memory_limit_kb = EXCLUDED.default_memory_limit_kb,
            max_cpu_time_limit_ms = EXCLUDED.max_cpu_time_limit_ms,
            max_cpu_limit_cores = EXCLUDED.max_cpu_limit_cores,
            max_memory_limit_kb = EXCLUDED.max_memory_limit_kb,
            is_active = TRUE,
            updated_at = now()
        RETURNING *
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(&manifest.language.slug)
    .bind(&manifest.language.display_name)
    .bind(&manifest.language.version)
    .bind(&manifest.image.reference)
    .bind(compile_cmd)
    .bind(run_cmd)
    .bind(&manifest.commands.source_filename)
    .bind(manifest.commands.compile_time_limit_ms)
    .bind(manifest.limits.default_cpu_time_limit_ms)
    .bind(manifest.limits.default_cpu_limit_cores)
    .bind(manifest.limits.default_memory_limit_kb)
    .bind(manifest.limits.max_cpu_time_limit_ms)
    .bind(manifest.limits.max_cpu_limit_cores)
    .bind(manifest.limits.max_memory_limit_kb)
    .fetch_one(pool)
    .await?;

    if let Some(nats) = nats {
        let payload = serde_json::json!({
            "language_id": language.id,
            "slug": language.slug,
            "action": "registered",
        });
        let _ = nats.publish("codexec.control.plugin_updated", payload.to_string().into()).await;
    }

    Ok(language)
}

pub async fn set_active(
    pool: &PgPool,
    nats: Option<&async_nats::Client>,
    slug: &str,
    is_active: bool,
) -> Result<Option<Language>, RegistryError> {
    let language = sqlx::query_as::<_, Language>(
        "UPDATE languages SET is_active = $1, updated_at = now() WHERE slug = $2 RETURNING *",
    )
    .bind(is_active)
    .bind(slug)
    .fetch_optional(pool)
    .await?;

    if let (Some(lang), Some(nats)) = (&language, nats) {
        let payload = serde_json::json!({
            "language_id": lang.id,
            "slug": lang.slug,
            "action": if is_active { "activated" } else { "deactivated" },
        });
        let _ = nats.publish("codexec.control.plugin_updated", payload.to_string().into()).await;
    }

    Ok(language)
}

/// A hard delete, distinct from `set_active(..., false)` — removes the
/// plugin definition entirely rather than just hiding it from new
/// submissions. `languages.id` has no `ON DELETE` behavior on
/// `submissions.language_id` (defaults to `NO ACTION`), so this correctly
/// fails with a foreign-key violation for any plugin that has ever been
/// submitted to; that case is surfaced as `RegistryError::InUse` so the
/// caller can return a clear 409 instead of a generic 500. Still notifies
/// workers via the control subject on success, so a cached copy is evicted
/// immediately rather than waiting on the periodic refresh.
pub async fn delete_language(
    pool: &PgPool,
    nats: Option<&async_nats::Client>,
    slug: &str,
) -> Result<bool, RegistryError> {
    let result = sqlx::query("DELETE FROM languages WHERE slug = $1").bind(slug).execute(pool).await;
    let deleted = match result {
        Ok(r) => r.rows_affected() > 0,
        Err(sqlx::Error::Database(db_err)) if db_err.code().as_deref() == Some("23503") => {
            return Err(RegistryError::InUse);
        }
        Err(e) => return Err(e.into()),
    };

    if deleted {
        if let Some(nats) = nats {
            let payload = serde_json::json!({ "slug": slug, "action": "deleted" });
            let _ = nats.publish("codexec.control.plugin_updated", payload.to_string().into()).await;
        }
    }

    Ok(deleted)
}
