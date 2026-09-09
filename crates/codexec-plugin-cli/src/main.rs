use clap::{Parser, Subcommand, ValueEnum};
use codexec_common::registry::{self, PluginManifest};
use codexec_exec_engine::image::{self, ImageSource};
use sqlx::postgres::PgPoolOptions;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "codexec-plugin-cli", about = "Register and manage codexec language plugins")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy, ValueEnum, PartialEq, Eq)]
enum SourceArg {
    /// Pull from a real registry (docker.io, a self-hosted registry, etc).
    Registry,
    /// Read directly from a local Docker daemon's image store - for a
    /// custom-built plugin image you haven't pushed anywhere yet.
    DockerDaemon,
}

impl From<SourceArg> for ImageSource {
    fn from(s: SourceArg) -> Self {
        match s {
            SourceArg::Registry => ImageSource::Registry,
            SourceArg::DockerDaemon => ImageSource::DockerDaemon,
        }
    }
}

#[derive(Subcommand)]
enum Command {
    /// Register or update a language plugin from a manifest file.
    Register {
        #[arg(long)]
        manifest: String,
        #[arg(long, value_enum, default_value_t = SourceArg::Registry)]
        source: SourceArg,
        /// Re-pull and re-unpack the image even if it's already cached
        /// (e.g. a moved tag, or a custom image rebuilt under the same
        /// name). Without this, an already-present image is left as-is.
        #[arg(long)]
        force: bool,
    },
    /// Pre-pull+unpack a single image into the shared rootfs cache,
    /// without touching Postgres/NATS at all. Useful for fleet
    /// provisioning scripts that only have a list of image refs (e.g.
    /// from `GET /admin/languages`), not full plugin manifests, and for
    /// hosts that shouldn't need DB credentials just to warm the image
    /// cache before `codexec-worker` starts accepting work.
    PullImage {
        #[arg(long)]
        image_ref: String,
        #[arg(long, value_enum, default_value_t = SourceArg::Registry)]
        source: SourceArg,
        #[arg(long)]
        force: bool,
    },
    /// Activate a previously registered language.
    Activate {
        #[arg(long)]
        slug: String,
    },
    /// Deactivate a language so new submissions against it are rejected.
    Deactivate {
        #[arg(long)]
        slug: String,
    },
    /// List all registered languages.
    List,
}

/// Pre-pulls and unpacks an image into the shared rootfs cache ahead of
/// submission time (via `skopeo` + `umoci` - see
/// `codexec_exec_engine::image`), so per-submission latency never includes
/// a pull. Must write to the exact `IMAGE_CACHE_ROOT` the worker reads
/// from - both default to the same path, but keep them in sync if you
/// override one. Idempotent unless `force` is set - see
/// `image::pull_and_unpack`.
async fn pull_image(image_ref: &str, source: ImageSource, force: bool) -> anyhow::Result<()> {
    let cache_root: PathBuf =
        std::env::var("IMAGE_CACHE_ROOT").unwrap_or_else(|_| "/var/lib/codexec/images".to_string()).into();
    let outcome = image::pull_and_unpack(&cache_root, image_ref, source, force).await?;
    if outcome.pulled {
        println!("pulled {image_ref} (source={source:?})");
    } else {
        println!("{image_ref} already present in cache, skipping pull (use --force to refresh)");
    }
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    // PullImage is a pure image-cache operation - deliberately doesn't
    // need DATABASE_URL/NATS_URL at all, so a fleet-provisioning host can
    // warm the image cache without DB credentials.
    if let Command::PullImage { image_ref, source, force } = &cli.command {
        pull_image(image_ref, (*source).into(), *force).await?;
        return Ok(());
    }

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let nats_url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string());

    let pool = PgPoolOptions::new().max_connections(2).connect(&database_url).await?;
    let nats = async_nats::connect(&nats_url).await.ok();
    if nats.is_none() {
        tracing::warn!("could not connect to NATS at {nats_url}; live workers will not be notified");
    }

    match cli.command {
        Command::Register { manifest, source, force } => {
            let contents = std::fs::read_to_string(&manifest)?;
            let manifest = PluginManifest::from_toml_str(&contents)?;
            pull_image(&manifest.image.reference, source.into(), force).await?;
            let language = registry::register_language(&pool, nats.as_ref(), &manifest).await?;
            println!("registered {} ({}) -> {}", language.slug, language.version, language.image_ref);
        }
        Command::PullImage { .. } => unreachable!("handled above"),
        Command::Activate { slug } => {
            match registry::set_active(&pool, nats.as_ref(), &slug, true).await? {
                Some(lang) => println!("activated {}", lang.slug),
                None => println!("no such language: {slug}"),
            }
        }
        Command::Deactivate { slug } => {
            match registry::set_active(&pool, nats.as_ref(), &slug, false).await? {
                Some(lang) => println!("deactivated {}", lang.slug),
                None => println!("no such language: {slug}"),
            }
        }
        Command::List => {
            let languages: Vec<codexec_common::models::Language> =
                sqlx::query_as("SELECT * FROM languages ORDER BY slug").fetch_all(&pool).await?;
            for lang in languages {
                println!(
                    "{:<12} v{:<10} active={:<5} image={}",
                    lang.slug, lang.version, lang.is_active, lang.image_ref
                );
            }
        }
    }

    Ok(())
}
