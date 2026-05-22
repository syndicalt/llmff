use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use clap::{Parser, Subcommand};
use llmff_core::backend::MockBackend;
use llmff_core::engine::{Engine, RunOptions};
use llmff_core::manifest::Manifest;

#[derive(Debug, Parser)]
#[command(name = "llmff", version)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Run {
        manifest: PathBuf,
        #[arg(long)]
        trace: Option<PathBuf>,
    },
    Inspect {
        manifest: PathBuf,
    },
    Backends {
        #[command(subcommand)]
        command: BackendsCommand,
    },
    Stages {
        #[command(subcommand)]
        command: StagesCommand,
    },
}

#[derive(Debug, Subcommand)]
enum StagesCommand {
    List,
}

#[derive(Debug, Subcommand)]
enum BackendsCommand {
    List,
}

pub async fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Run { manifest, trace } => run_manifest(&manifest, trace).await?,
        Command::Inspect { manifest } => {
            let source = std::fs::read_to_string(&manifest)?;
            let manifest = Manifest::from_yaml_str(&source)?;
            llmff_core::graph::Graph::from_manifest(manifest)?;
            println!("ok");
        }
        Command::Backends {
            command: BackendsCommand::List,
        } => {
            println!("mock:bad");
            println!("mock:good");
            println!("openai-compatible");
        }
        Command::Stages {
            command: StagesCommand::List,
        } => {
            println!("load");
            println!("system");
            println!("template");
            println!("infer");
            println!("validate_json");
            println!("repair");
            println!("route");
            println!("tool");
            println!("write");
        }
    }

    Ok(())
}

async fn run_manifest(manifest_path: &Path, trace: Option<PathBuf>) -> Result<()> {
    let source = std::fs::read_to_string(manifest_path)?;
    let manifest = Manifest::from_yaml_str(&source)?;
    let cwd = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let bad = std::env::var("LLMFF_MOCK_BAD_RESPONSE").unwrap_or_else(|_| "{}".to_string());
    let good = std::env::var("LLMFF_MOCK_GOOD_RESPONSE").unwrap_or_else(|_| bad.clone());
    let engine = Engine::new()
        .with_backend("mock:bad", Arc::new(MockBackend::new("mock:bad", bad)))
        .with_backend(
            "mock:good",
            Arc::new(MockBackend::new("mock:good", good.clone())),
        )
        .with_backend("mock:json", Arc::new(MockBackend::new("mock:json", good)));
    let options = RunOptions {
        run_id: "cli-run".to_string(),
        trace_path: trace,
    };

    engine
        .run_manifest_with_options(manifest, cwd, options)
        .await?;

    Ok(())
}
