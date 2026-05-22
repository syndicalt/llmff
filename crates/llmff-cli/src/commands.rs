use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use clap::{Parser, Subcommand};
use llmff_core::backend::{MockBackend, OpenAiCompatibleBackend};
use llmff_core::engine::{Engine, RunOptions};
use llmff_core::manifest::Manifest;

#[derive(Debug, Clone, PartialEq, Eq)]
struct AliasValue {
    alias: String,
    value: String,
}

#[derive(Debug, Parser)]
#[command(name = "llmff", version)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Run {
        manifest: Option<PathBuf>,
        #[arg(short = 'i', long = "input")]
        input: Option<PathBuf>,
        #[arg(short = 'g', long = "graph")]
        graph: Option<String>,
        #[arg(long)]
        trace: Option<PathBuf>,
        #[arg(long = "backend")]
        backend: Vec<String>,
        #[arg(long = "api-key-env")]
        api_key_env: Vec<String>,
        #[arg(long = "api-key")]
        api_key: Vec<String>,
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
        Command::Run {
            manifest,
            input,
            graph,
            trace,
            backend,
            api_key_env,
            api_key,
        } => run_pipeline(manifest, input, graph, trace, backend, api_key_env, api_key).await?,
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

async fn run_pipeline(
    manifest_path: Option<PathBuf>,
    input_path: Option<PathBuf>,
    inline_graph: Option<String>,
    trace: Option<PathBuf>,
    backend: Vec<String>,
    api_key_env: Vec<String>,
    api_key: Vec<String>,
) -> Result<()> {
    let (manifest, cwd) = load_run_manifest(manifest_path, input_path, inline_graph)?;
    let bad = std::env::var("LLMFF_MOCK_BAD_RESPONSE").unwrap_or_else(|_| "{}".to_string());
    let good = std::env::var("LLMFF_MOCK_GOOD_RESPONSE").unwrap_or_else(|_| bad.clone());
    let mut engine = Engine::new()
        .with_backend("mock:bad", Arc::new(MockBackend::new("mock:bad", bad)))
        .with_backend(
            "mock:good",
            Arc::new(MockBackend::new("mock:good", good.clone())),
        )
        .with_backend("mock:json", Arc::new(MockBackend::new("mock:json", good)));

    let api_key_env = parse_alias_value_map(api_key_env)?;
    let api_key = parse_alias_value_map(api_key)?;
    for backend in parse_alias_value_list(backend)? {
        let key = api_key
            .get(&backend.alias)
            .cloned()
            .map(Ok)
            .or_else(|| resolve_api_key_env(&api_key_env, &backend.alias))
            .transpose()?
            .unwrap_or_default();
        engine = engine.with_backend(
            backend.alias,
            Arc::new(OpenAiCompatibleBackend::new(backend.value, key)),
        );
    }

    let options = RunOptions {
        run_id: "cli-run".to_string(),
        trace_path: trace,
    };

    engine
        .run_manifest_with_options(manifest, &cwd, options)
        .await?;

    Ok(())
}

fn load_run_manifest(
    manifest_path: Option<PathBuf>,
    input_path: Option<PathBuf>,
    inline_graph: Option<String>,
) -> Result<(Manifest, PathBuf)> {
    match (manifest_path, inline_graph) {
        (Some(_), Some(_)) => anyhow::bail!("provide either manifest or --graph, not both"),
        (None, None) => anyhow::bail!("provide either manifest or --graph"),
        (Some(path), None) => {
            let source = std::fs::read_to_string(&path)?;
            let manifest = Manifest::from_yaml_str(&source)?;
            let cwd = path.parent().unwrap_or_else(|| Path::new(".")).to_path_buf();
            Ok((manifest, cwd))
        }
        (None, Some(graph)) => {
            let input = input_path.map(|path| path.to_string_lossy().to_string());
            let manifest = Manifest::from_inline_graph(&graph, input)?;
            Ok((manifest, std::env::current_dir()?))
        }
    }
}

fn parse_alias_value(source: &str) -> Result<AliasValue> {
    let (alias, value) = source
        .split_once('=')
        .ok_or_else(|| anyhow::anyhow!("expected alias=value, got `{source}`"))?;
    if alias.is_empty() {
        anyhow::bail!("expected non-empty alias in alias=value, got `{source}`");
    }
    if value.is_empty() {
        anyhow::bail!("expected non-empty value in alias=value, got `{source}`");
    }

    Ok(AliasValue {
        alias: alias.to_string(),
        value: value.to_string(),
    })
}

fn parse_alias_value_list(sources: Vec<String>) -> Result<Vec<AliasValue>> {
    sources
        .into_iter()
        .map(|source| parse_alias_value(&source))
        .collect()
}

fn parse_alias_value_map(
    sources: Vec<String>,
) -> Result<std::collections::BTreeMap<String, String>> {
    parse_alias_value_list(sources).map(|pairs| {
        pairs
            .into_iter()
            .map(|pair| (pair.alias, pair.value))
            .collect()
    })
}

fn resolve_api_key_env(
    api_key_env: &std::collections::BTreeMap<String, String>,
    alias: &str,
) -> Option<Result<String>> {
    api_key_env.get(alias).map(|name| {
        std::env::var(name)
            .map_err(|_| anyhow::anyhow!("api key env `{name}` for backend `{alias}` is not set"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_backend_config_parses_alias_value_pair() {
        let pair = parse_alias_value("openai=https://api.example.test/v1").unwrap();

        assert_eq!(pair.alias, "openai");
        assert_eq!(pair.value, "https://api.example.test/v1");
    }

    #[test]
    fn cli_backend_config_rejects_malformed_pair() {
        let error = parse_alias_value("openai").unwrap_err().to_string();

        assert!(error.contains("expected alias=value"));
    }
}
