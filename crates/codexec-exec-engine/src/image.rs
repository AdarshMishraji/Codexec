use crate::error::EngineError;
use containerd_client::services::v1::{
    content_client::ContentClient, images_client::ImagesClient, GetImageRequest, ReadContentRequest,
};
use containerd_client::{tonic::transport::Channel, tonic::Request, with_namespace};
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
struct OciDescriptor {
    digest: String,
}

#[derive(Deserialize)]
struct OciManifest {
    config: OciDescriptor,
}

#[derive(Deserialize)]
struct OciRootfs {
    diff_ids: Vec<String>,
}

#[derive(Deserialize)]
struct OciImageConfig {
    rootfs: OciRootfs,
}

#[derive(Deserialize)]
struct OciPlatform {
    architecture: String,
    os: String,
}

#[derive(Deserialize)]
struct OciIndexEntry {
    digest: String,
    platform: Option<OciPlatform>,
}

#[derive(Deserialize)]
struct OciIndex {
    manifests: Vec<OciIndexEntry>,
}

/// Maps Rust's `std::env::consts::ARCH` to the OCI platform architecture
/// string used in image index/manifest-list entries (e.g. "aarch64" ->
/// "arm64"). Falls back to the Rust name unchanged for architectures where
/// the two already agree (e.g. "amd64" callers would need to map from
/// "x86_64" - handled explicitly below).
fn oci_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        other => other,
    }
}

/// Resolves an already-pulled image reference to its chain ID, per the
/// OCI image spec algorithm: chain[0]=diff[0], chain[i]=sha256("<chain[i-1]>
/// <diff[i]>"). The chain ID is what `Snapshots.Prepare` needs as `parent`
/// to hand back a usable rootfs for a new container.
pub async fn resolve_chain_id(channel: Channel, ns: &str, image_ref: &str) -> Result<String, EngineError> {
    let mut images = ImagesClient::new(channel.clone());
    let resp = images
        .get(with_namespace!(GetImageRequest { name: image_ref.into() }, ns))
        .await?
        .into_inner();
    let target = resp
        .image
        .ok_or_else(|| EngineError::ImageNotFound(image_ref.to_string()))?
        .target
        .ok_or_else(|| EngineError::Internal(format!("image {image_ref} has no target descriptor")))?;

    let mut content = ContentClient::new(channel.clone());
    let root_bytes = read_content_fully(&mut content, ns, &target.digest).await?;

    // A tag like "busybox:latest" commonly resolves to a multi-arch image
    // INDEX (or Docker manifest list) rather than a single-platform
    // manifest directly — it has a `manifests` array, not `config`. Detect
    // that case and follow it down to the manifest for our own platform
    // before looking for `config`/`rootfs.diff_ids`.
    let root_value: serde_json::Value = serde_json::from_slice(&root_bytes)?;
    let manifest_bytes = if root_value.get("manifests").is_some() {
        let index: OciIndex = serde_json::from_value(root_value)?;
        let arch = oci_arch();
        let entry = index
            .manifests
            .iter()
            .find(|m| m.platform.as_ref().is_some_and(|p| p.architecture == arch && p.os == "linux"))
            .ok_or_else(|| {
                EngineError::Internal(format!("image {image_ref} has no manifest for platform linux/{arch}"))
            })?;
        read_content_fully(&mut content, ns, &entry.digest).await?
    } else {
        root_bytes
    };

    let manifest: OciManifest = serde_json::from_slice(&manifest_bytes)?;
    let config_bytes = read_content_fully(&mut content, ns, &manifest.config.digest).await?;
    let config: OciImageConfig = serde_json::from_slice(&config_bytes)?;

    let mut chain: Option<String> = None;
    for diff_id in &config.rootfs.diff_ids {
        chain = Some(match chain {
            None => diff_id.clone(),
            Some(prev) => {
                let mut h = Sha256::new();
                h.update(format!("{prev} {diff_id}").as_bytes());
                format!("sha256:{:x}", h.finalize())
            }
        });
    }
    chain.ok_or_else(|| EngineError::Internal(format!("image {image_ref} has empty rootfs")))
}

async fn read_content_fully(
    client: &mut ContentClient<Channel>,
    ns: &str,
    digest: &str,
) -> Result<Vec<u8>, EngineError> {
    let req = with_namespace!(ReadContentRequest { digest: digest.into(), offset: 0, size: 0 }, ns);
    let mut stream = client.read(req).await?.into_inner();
    let mut buf = Vec::new();
    while let Some(chunk) = stream.message().await? {
        buf.extend_from_slice(&chunk.data);
    }
    Ok(buf)
}
