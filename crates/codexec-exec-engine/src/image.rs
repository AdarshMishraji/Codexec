use crate::error::EngineError;
use std::path::{Path, PathBuf};
use tokio::process::Command;

pub struct PullOutcome {
    pub rootfs: PathBuf,
    /// `false` if this was a no-op because the image was already cached.
    pub pulled: bool,
}

/// Where a plugin image gets pulled from. `Registry` covers any real
/// registry pull (`docker.io/...`, a self-hosted registry, ECR/GHCR/etc);
/// `DockerDaemon` reads directly from a local Docker daemon's image store
/// (no registry needed) - useful for local dev/testing of custom-built
/// plugin images before they're pushed anywhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageSource {
    Registry,
    DockerDaemon,
}

impl ImageSource {
    fn skopeo_source(self, image_ref: &str) -> String {
        match self {
            ImageSource::Registry => format!("docker://{image_ref}"),
            ImageSource::DockerDaemon => format!("docker-daemon:{image_ref}"),
        }
    }
}

/// Deterministic, filesystem-safe cache key for an image reference. Two
/// different refs must never collide, and re-registering the same ref
/// must always resolve back to the same directory (so re-pulling refreshes
/// it in place rather than accumulating stale copies).
fn image_key(image_ref: &str) -> String {
    image_ref.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' }).collect()
}

fn oci_layout_dir(cache_root: &Path, image_ref: &str) -> PathBuf {
    cache_root.join(format!("{}.ocilayout", image_key(image_ref)))
}

fn bundle_dir(cache_root: &Path, image_ref: &str) -> PathBuf {
    cache_root.join(format!("{}.bundle", image_key(image_ref)))
}

/// Marks a bundle directory as fully unpacked - guards against a worker
/// treating a partially-written directory (crash mid-unpack) as ready.
fn complete_marker(cache_root: &Path, image_ref: &str) -> PathBuf {
    bundle_dir(cache_root, image_ref).join(".codexec-complete")
}

/// The engine's only entry point into image handling: is this image ready
/// to run against? Never pulls - `codexec-plugin-cli` (`pull_and_unpack`,
/// below) is the only thing that populates the cache, at registration
/// time, so per-submission latency never includes a pull.
pub async fn ensure_present(cache_root: &Path, image_ref: &str) -> Result<PathBuf, EngineError> {
    let rootfs = bundle_dir(cache_root, image_ref).join("rootfs");
    if tokio::fs::try_exists(complete_marker(cache_root, image_ref)).await.unwrap_or(false) {
        Ok(rootfs)
    } else {
        Err(EngineError::ImageNotFound(image_ref.to_string()))
    }
}

async fn run(program: &str, args: &[&str]) -> Result<(), EngineError> {
    let output = Command::new(program)
        .args(args)
        .output()
        .await
        .map_err(|e| EngineError::ImagePull(format!("failed to exec {program}: {e}")))?;
    if !output.status.success() {
        return Err(EngineError::ImagePull(format!(
            "{program} {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(())
}

/// Pulls `image_ref` (via `skopeo`) and unpacks it (via `umoci`) into a
/// plain rootfs directory under `cache_root`, shared read-only across
/// every future submission for this image.
///
/// Idempotent by default: if this exact ref was already pulled+unpacked
/// (the completion marker is present), this is a no-op that just returns
/// the existing rootfs path - re-running `codexec-plugin-cli register`
/// to update a language's limits/commands shouldn't re-download a
/// multi-hundred-MB image every time. Pass `force: true` to explicitly
/// refresh (a moved tag, or a custom image rebuilt under the same name).
pub async fn pull_and_unpack(
    cache_root: &Path,
    image_ref: &str,
    source: ImageSource,
    force: bool,
) -> Result<PullOutcome, EngineError> {
    let rootfs = bundle_dir(cache_root, image_ref).join("rootfs");
    if !force && tokio::fs::try_exists(complete_marker(cache_root, image_ref)).await.unwrap_or(false) {
        return Ok(PullOutcome { rootfs, pulled: false });
    }

    tokio::fs::create_dir_all(cache_root).await?;

    let layout_dir = oci_layout_dir(cache_root, image_ref);
    let bundle = bundle_dir(cache_root, image_ref);
    // Clean slate: umoci refuses to unpack into a non-empty directory, and
    // we want a stale rootfs fully gone rather than merged with a new one.
    let _ = tokio::fs::remove_dir_all(&layout_dir).await;
    let _ = tokio::fs::remove_dir_all(&bundle).await;

    let src = source.skopeo_source(image_ref);
    let dest = format!("oci:{}:image", layout_dir.display());
    run("skopeo", &["copy", &src, &dest]).await?;

    let image_arg = format!("{}:image", layout_dir.display());
    let bundle_arg = bundle.display().to_string();
    run("umoci", &["unpack", "--image", &image_arg, &bundle_arg]).await?;

    tokio::fs::write(complete_marker(cache_root, image_ref), b"").await?;
    Ok(PullOutcome { rootfs: bundle.join("rootfs"), pulled: true })
}
