use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use clap::{Parser, Subcommand};
use llmff_core::backend::{MockBackend, OllamaBackend, OpenAiCompatibleBackend};
use llmff_core::engine::{Engine, RunOptions, SchedulerMode};
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
        #[arg(long)]
        parallel: bool,
        #[arg(long = "backend")]
        backend: Vec<String>,
        #[arg(long = "ollama")]
        ollama: Vec<String>,
        #[arg(long = "api-key-env")]
        api_key_env: Vec<String>,
        #[arg(long = "api-key")]
        api_key: Vec<String>,
    },
    Inspect {
        manifest: Option<PathBuf>,
        #[arg(short = 'i', long = "input")]
        input: Option<PathBuf>,
        #[arg(short = 'g', long = "graph")]
        graph: Option<String>,
        #[arg(long = "backend")]
        backend: Vec<String>,
        #[arg(long = "ollama")]
        ollama: Vec<String>,
        #[arg(long = "api-key-env")]
        api_key_env: Vec<String>,
        #[arg(long = "api-key")]
        api_key: Vec<String>,
    },
    Backends {
        #[command(subcommand)]
        command: BackendsCommand,
    },
    Stages {
        #[command(subcommand)]
        command: StagesCommand,
    },
    Trace {
        path: PathBuf,
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
            parallel,
            backend,
            ollama,
            api_key_env,
            api_key,
        } => {
            run_pipeline(
                manifest,
                input,
                graph,
                trace,
                parallel,
                backend,
                ollama,
                api_key_env,
                api_key,
            )
            .await?
        }
        Command::Inspect {
            manifest,
            input,
            graph,
            backend,
            ollama,
            api_key_env,
            api_key,
        } => {
            let (manifest, _) = load_pipeline_manifest(manifest, input, graph)?;
            let engine = build_engine(backend, ollama, api_key_env, api_key)?;
            engine.validate_manifest(manifest)?;
            println!("ok");
        }
        Command::Backends {
            command: BackendsCommand::List,
        } => {
            println!("mock:bad");
            println!("mock:good");
            println!("ollama");
            println!("openai-compatible");
        }
        Command::Stages {
            command: StagesCommand::List,
        } => {
            println!("load");
            println!("cache");
            println!("system");
            println!("template");
            println!("retrieve");
            println!("infer");
            println!("validate_json");
            println!("repair");
            println!("route");
            println!("tool");
            println!("write");
        }
        Command::Trace { path } => summarize_trace(&path)?,
    }

    Ok(())
}

fn summarize_trace(path: &Path) -> Result<()> {
    let source = std::fs::read_to_string(path)?;
    for (index, line) in source.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let event: serde_json::Value = serde_json::from_str(line).map_err(|error| {
            anyhow::anyhow!("invalid trace JSON on line {}: {error}", index + 1)
        })?;
        if let Some(summary) = summarize_trace_event(&event) {
            println!("{summary}");
        }
    }

    Ok(())
}

fn summarize_trace_event(event: &serde_json::Value) -> Option<String> {
    match event.get("event")?.as_str()? {
        "run_finished" => Some(format!(
            "run {} {}",
            string_field(event, "run_id").unwrap_or("unknown"),
            string_field(event, "status").unwrap_or("unknown")
        )),
        "stage_finished" => {
            let mut parts = vec![
                string_field(event, "stage_id")
                    .unwrap_or("unknown")
                    .to_string(),
                string_field(event, "op").unwrap_or("unknown").to_string(),
                string_field(event, "status")
                    .unwrap_or("unknown")
                    .to_string(),
                format!("{}ms", integer_field(event, "duration_ms").unwrap_or(0)),
            ];

            push_string_metadata(&mut parts, event, "model");
            push_string_metadata(&mut parts, event, "backend");
            push_string_metadata(&mut parts, event, "provider_model");
            if let Some(total) = integer_field(event, "total_tokens") {
                parts.push(format!("usage={total}"));
            }
            if let Some(prompt) = integer_field(event, "prompt_tokens") {
                parts.push(format!("prompt_tokens={prompt}"));
            }
            if let Some(completion) = integer_field(event, "completion_tokens") {
                parts.push(format!("completion_tokens={completion}"));
            }
            if let Some(count) = event
                .get("validation_errors")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len)
            {
                parts.push(format!("validation_errors={count}"));
            }
            push_string_metadata(&mut parts, event, "tool_kind");
            push_string_metadata(&mut parts, event, "tool_target");
            push_string_metadata(&mut parts, event, "output_path");
            if let Some(cache_hit) = bool_field(event, "cache_hit") {
                parts.push(format!("cache_hit={cache_hit}"));
            }
            push_string_metadata(&mut parts, event, "cache_path");

            Some(parts.join(" "))
        }
        _ => None,
    }
}

fn push_string_metadata(parts: &mut Vec<String>, event: &serde_json::Value, name: &str) {
    if let Some(value) = string_field(event, name) {
        parts.push(format!("{name}={value}"));
    }
}

fn string_field<'a>(event: &'a serde_json::Value, name: &str) -> Option<&'a str> {
    event.get(name).and_then(serde_json::Value::as_str)
}

fn integer_field(event: &serde_json::Value, name: &str) -> Option<u64> {
    event.get(name).and_then(serde_json::Value::as_u64)
}

fn bool_field(event: &serde_json::Value, name: &str) -> Option<bool> {
    event.get(name).and_then(serde_json::Value::as_bool)
}

async fn run_pipeline(
    manifest_path: Option<PathBuf>,
    input_path: Option<PathBuf>,
    inline_graph: Option<String>,
    trace: Option<PathBuf>,
    parallel: bool,
    backend: Vec<String>,
    ollama: Vec<String>,
    api_key_env: Vec<String>,
    api_key: Vec<String>,
) -> Result<()> {
    let (manifest, cwd) = load_pipeline_manifest(manifest_path, input_path, inline_graph)?;
    let engine = build_engine(backend, ollama, api_key_env, api_key)?;

    let options = RunOptions {
        run_id: "cli-run".to_string(),
        trace_path: trace,
        scheduler: if parallel {
            SchedulerMode::Parallel
        } else {
            SchedulerMode::Sequential
        },
    };

    engine
        .run_manifest_with_options(manifest, &cwd, options)
        .await?;

    Ok(())
}

fn build_engine(
    backend: Vec<String>,
    ollama: Vec<String>,
    api_key_env: Vec<String>,
    api_key: Vec<String>,
) -> Result<Engine> {
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
    for backend in parse_alias_value_list(ollama)? {
        engine = engine.with_backend(backend.alias, Arc::new(OllamaBackend::new(backend.value)));
    }

    Ok(engine)
}

fn load_pipeline_manifest(
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
            let cwd = path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf();
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
