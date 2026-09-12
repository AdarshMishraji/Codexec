use crate::error::ApiError;
use axum::extract::Path;
use axum::Json;
use codexec_common::registry::PluginManifest;
use serde::Serialize;

/// Manifests for the plugins this repo ships Dockerfiles for (see
/// `plugins/*/plugin.toml`), whose images are published publicly on Docker
/// Hub. Embedded at compile time — same reasoning as the dashboard/admin
/// HTML assets — so the admin portal can offer "start from an existing
/// plugin" with zero runtime dependency on the `plugins/` source directory
/// being present next to the deployed binary.
const TEMPLATE_TOMLS: &[&str] = &[
    include_str!("../../../plugins/c/plugin.toml"),
    include_str!("../../../plugins/cpp/plugin.toml"),
    include_str!("../../../plugins/csharp/plugin.toml"),
    include_str!("../../../plugins/dart/plugin.toml"),
    include_str!("../../../plugins/go/plugin.toml"),
    include_str!("../../../plugins/java/plugin.toml"),
    include_str!("../../../plugins/javascript/plugin.toml"),
    include_str!("../../../plugins/kotlin/plugin.toml"),
    include_str!("../../../plugins/python3/plugin.toml"),
    include_str!("../../../plugins/rust/plugin.toml"),
    include_str!("../../../plugins/swift/plugin.toml"),
    include_str!("../../../plugins/typescript/plugin.toml"),
];

fn all_templates() -> Vec<PluginManifest> {
    TEMPLATE_TOMLS
        .iter()
        .filter_map(|raw| match PluginManifest::from_toml_str(raw) {
            Ok(m) => Some(m),
            Err(e) => {
                tracing::error!(error = %e, "failed to parse a built-in plugin template - skipping it");
                None
            }
        })
        .collect()
}

#[derive(Serialize)]
pub struct TemplateSummary {
    pub slug: String,
    pub display_name: String,
    pub version: String,
}

pub async fn list() -> Json<Vec<TemplateSummary>> {
    let mut items: Vec<TemplateSummary> = all_templates()
        .into_iter()
        .map(|m| TemplateSummary {
            slug: m.language.slug,
            display_name: m.language.display_name,
            version: m.language.version,
        })
        .collect();
    items.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    Json(items)
}

pub async fn get_one(Path(slug): Path<String>) -> Result<Json<PluginManifest>, ApiError> {
    all_templates()
        .into_iter()
        .find(|m| m.language.slug == slug)
        .map(Json)
        .ok_or_else(|| ApiError::not_found(format!("no built-in template for '{slug}'")))
}
