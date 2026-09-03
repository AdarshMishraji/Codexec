use clap::{Parser, Subcommand};
use codexec_common::registry::{self, PluginManifest};
use sqlx::postgres::PgPoolOptions;

#[derive(Parser)]
#[command(name = "codexec-plugin-cli", about = "Register and manage codexec language plugins")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Register or update a language plugin from a manifest file.
    Register {
        #[arg(long)]
        manifest: String,
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

/// Pre-pulls an image into containerd's content/image store ahead of
/// submission time, so per-submission latency only involves container
/// create+start+wait+delete, not image pull. Shells out to `ctr` (the
/// standard containerd CLI, present alongside containerd in the worker's
/// container image) rather than reimplementing image transfer over gRPC —
/// this is an operator-run, dev-tooling path, not a hot path, so the
/// simplicity of shelling out to the reference tool wins over a bespoke
/// Rust implementation of resolve/fetch/unpack.
fn pull_image(image_ref: &str) -> anyhow::Result<()> {
    let socket = std::env::var("CONTAINERD_SOCKET_PATH")
        .unwrap_or_else(|_| "/run/containerd/containerd.sock".to_string());
    let namespace = std::env::var("CONTAINERD_NAMESPACE").unwrap_or_else(|_| "codexec".to_string());

    println!("pulling {image_ref} into containerd (namespace={namespace})...");
    let status = std::process::Command::new("ctr")
        .args(["-a", &socket, "-n", &namespace, "image", "pull", image_ref])
        .status();

    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => anyhow::bail!("ctr image pull exited with status {s}"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::warn!(
                "`ctr` not found on PATH; skipping image pre-pull for {image_ref}. \
                 Run this command inside the containerd-equipped worker container \
                 (e.g. `docker compose exec worker codexec-plugin-cli register ...`), \
                 or pull the image manually before submissions against this language will work."
            );
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let nats_url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string());

    let pool = PgPoolOptions::new().max_connections(2).connect(&database_url).await?;
    let nats = async_nats::connect(&nats_url).await.ok();
    if nats.is_none() {
        tracing::warn!("could not connect to NATS at {nats_url}; live workers will not be notified");
    }

    match cli.command {
        Command::Register { manifest } => {
            let contents = std::fs::read_to_string(&manifest)?;
            let manifest = PluginManifest::from_toml_str(&contents)?;
            pull_image(&manifest.image.reference)?;
            let language = registry::register_language(&pool, nats.as_ref(), &manifest).await?;
            println!("registered {} ({}) -> {}", language.slug, language.version, language.image_ref);
        }
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
