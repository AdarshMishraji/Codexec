use codexec_common::models::Language;
use futures::StreamExt;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// In-memory cache of active languages, loaded at startup and kept fresh
/// two ways: a control-subject subscription for near-immediate updates
/// (never trusted as authoritative — it only tells us *which* row to
/// re-fetch from Postgres), and a periodic full refresh as a fallback for
/// a dropped control message (core NATS pub/sub has no redelivery).
pub struct PluginRegistry {
    inner: RwLock<HashMap<String, Language>>,
    pool: PgPool,
}

impl PluginRegistry {
    pub async fn load(pool: PgPool) -> Result<Arc<Self>, sqlx::Error> {
        let registry = Arc::new(Self { inner: RwLock::new(HashMap::new()), pool });
        registry.refresh_all().await?;
        Ok(registry)
    }

    pub async fn refresh_all(&self) -> Result<(), sqlx::Error> {
        let languages: Vec<Language> =
            sqlx::query_as("SELECT * FROM languages WHERE is_active").fetch_all(&self.pool).await?;
        let mut map = HashMap::new();
        for lang in languages {
            map.insert(lang.slug.clone(), lang);
        }
        *self.inner.write().await = map;
        Ok(())
    }

    pub async fn refresh_one(&self, slug: &str) {
        let language: Result<Option<Language>, _> =
            sqlx::query_as("SELECT * FROM languages WHERE slug = $1 AND is_active").bind(slug).fetch_optional(&self.pool).await;
        match language {
            Ok(Some(lang)) => {
                self.inner.write().await.insert(slug.to_string(), lang);
            }
            Ok(None) => {
                self.inner.write().await.remove(slug);
            }
            Err(e) => {
                tracing::warn!(slug, error = %e, "failed to refresh language from control-subject hint");
            }
        }
    }

    pub async fn get(&self, slug: &str) -> Option<Language> {
        self.inner.read().await.get(slug).cloned()
    }

    /// One-off direct DB lookup, bypassing the cache entirely. Used as a
    /// fallback right before giving up on a cache miss, since the cache is
    /// only eventually consistent (control-subject notification + a 60s
    /// periodic refresh) and can otherwise spuriously fail a submission
    /// for a language that was activated moments earlier. Also backfills
    /// the cache so the next lookup doesn't need to hit the DB again.
    pub async fn fetch_fresh(&self, slug: &str) -> Option<Language> {
        let language: Option<Language> =
            sqlx::query_as("SELECT * FROM languages WHERE slug = $1 AND is_active")
                .bind(slug)
                .fetch_optional(&self.pool)
                .await
                .ok()
                .flatten();
        if let Some(lang) = &language {
            self.inner.write().await.insert(slug.to_string(), lang.clone());
        }
        language
    }
}

pub fn spawn_control_subscriber(registry: Arc<PluginRegistry>, nats: async_nats::Client) {
    tokio::spawn(async move {
        let Ok(mut sub) = nats.subscribe("codexec.control.plugin_updated").await else {
            tracing::error!("failed to subscribe to codexec.control.plugin_updated");
            return;
        };
        while let Some(msg) = sub.next().await {
            if let Ok(payload) = serde_json::from_slice::<serde_json::Value>(&msg.payload) {
                if let Some(slug) = payload.get("slug").and_then(|v| v.as_str()) {
                    registry.refresh_one(slug).await;
                }
            }
        }
    });
}

pub fn spawn_periodic_refresh(registry: Arc<PluginRegistry>, interval: Duration) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        loop {
            ticker.tick().await;
            if let Err(e) = registry.refresh_all().await {
                tracing::warn!(error = %e, "periodic plugin registry refresh failed");
            }
        }
    });
}
