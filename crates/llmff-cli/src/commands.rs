use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use llmff_core::backend::{
    builtin_backend_families, CommandBackend, MockBackend, OllamaBackend, OpenAiCompatibleBackend,
};
use llmff_core::engine::{Engine, RunOptions, SchedulerMode};
use llmff_core::manifest::Manifest;
use llmff_core::plugin::{
    discover_plugin_backends, discover_plugin_manifests, validate_plugin_directory,
    validate_plugin_manifests,
};
use llmff_core::stage::builtin_stage_metadata;

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
        #[arg(long = "events")]
        events: Option<PathBuf>,
        #[arg(long)]
        parallel: bool,
        #[arg(long = "plugin-dir")]
        plugin_dir: Vec<PathBuf>,
        #[arg(long = "stream-stage")]
        stream_stage: Option<String>,
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
        #[arg(long = "plugin-dir")]
        plugin_dir: Vec<PathBuf>,
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
    Models {
        #[command(subcommand)]
        command: ModelsCommand,
    },
    Stages {
        #[command(subcommand)]
        command: StagesCommand,
    },
    Plugins {
        #[command(subcommand)]
        command: PluginsCommand,
    },
    Trace {
        path: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum StagesCommand {
    List {
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
}

#[derive(Debug, Subcommand)]
enum BackendsCommand {
    List {
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
        #[arg(long = "backend")]
        backend: Vec<String>,
        #[arg(long = "ollama")]
        ollama: Vec<String>,
        #[arg(long = "plugin-dir")]
        plugin_dir: Vec<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
enum ModelsCommand {
    List {
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
        #[arg(long = "backend")]
        backend: Vec<String>,
        #[arg(long = "ollama")]
        ollama: Vec<String>,
        #[arg(long = "plugin-dir")]
        plugin_dir: Vec<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
enum PluginsCommand {
    List {
        #[arg(long = "plugin-dir")]
        plugin_dir: PathBuf,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
    Validate {
        #[arg(long = "plugin-dir")]
        plugin_dir: PathBuf,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

pub async fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Run {
            manifest,
            input,
            graph,
            trace,
            events,
            parallel,
            plugin_dir,
            stream_stage,
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
                events,
                parallel,
                plugin_dir,
                stream_stage,
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
            plugin_dir,
            backend,
            ollama,
            api_key_env,
            api_key,
        } => {
            let (manifest, _) = load_pipeline_manifest(manifest, input, graph)?;
            let engine = build_engine(backend, ollama, api_key_env, api_key, &plugin_dir)?;
            engine.validate_manifest_with_plugin_dirs(manifest, &plugin_dir)?;
            println!("ok");
        }
        Command::Backends {
            command:
                BackendsCommand::List {
                    format,
                    backend,
                    ollama,
                    plugin_dir,
                },
        } => print_backend_families(format, backend, ollama, plugin_dir)?,
        Command::Models {
            command:
                ModelsCommand::List {
                    format,
                    backend,
                    ollama,
                    plugin_dir,
                },
        } => print_model_runtimes(format, backend, ollama, plugin_dir)?,
        Command::Stages {
            command: StagesCommand::List { format },
        } => print_stage_metadata(format)?,
        Command::Plugins {
            command: PluginsCommand::List { plugin_dir, format },
        } => print_plugin_manifests(&plugin_dir, format)?,
        Command::Plugins {
            command: PluginsCommand::Validate { plugin_dir, format },
        } => validate_plugins(&plugin_dir, format)?,
        Command::Trace { path } => summarize_trace(&path)?,
    }

    Ok(())
}

fn print_stage_metadata(format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Text => {
            for stage in builtin_stage_metadata() {
                println!("{}", stage.name);
            }
        }
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(builtin_stage_metadata())?
            );
        }
    }

    Ok(())
}

fn print_backend_families(
    format: OutputFormat,
    backend: Vec<String>,
    ollama: Vec<String>,
    plugin_dir: Vec<PathBuf>,
) -> Result<()> {
    let backends = backend_family_views(backend, ollama, plugin_dir)?;
    match format {
        OutputFormat::Text => {
            for backend in backends {
                let model_aliases = backend
                    .get("model_aliases")
                    .and_then(serde_json::Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                if model_aliases.is_empty() {
                    if let Some(name) = backend.get("name").and_then(serde_json::Value::as_str) {
                        println!("{name}");
                    }
                } else {
                    for alias in model_aliases {
                        if let Some(alias) = alias.as_str() {
                            println!("{alias}");
                        }
                    }
                }
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&backends)?);
        }
    }

    Ok(())
}

fn backend_family_views(
    backend: Vec<String>,
    ollama: Vec<String>,
    plugin_dir: Vec<PathBuf>,
) -> Result<Vec<serde_json::Value>> {
    let mut families = builtin_backend_families()
        .iter()
        .map(|backend| {
            serde_json::json!({
                "name": backend.name,
                "kind": backend.kind,
                "registration_flag": backend.registration_flag,
                "requires_api_key": backend.requires_api_key,
                "model_aliases": backend.model_aliases,
                "capabilities": backend.capabilities,
            })
        })
        .collect::<Vec<_>>();

    for backend in parse_alias_value_list(backend)? {
        families.push(serde_json::json!({
            "name": backend.alias,
            "kind": "openai-compatible",
            "registration_flag": format!("--backend {}=<base-url>", backend.alias),
            "requires_api_key": true,
            "model_aliases": [format!("{}:<model>", backend.alias)],
            "capabilities": [
                "chat-messages",
                "response-format-json",
                "sampling",
                "seed-control",
                "stop-sequences",
                "streaming-inference",
                "usage-metadata",
            ],
        }));
    }

    for backend in parse_alias_value_list(ollama)? {
        families.push(serde_json::json!({
            "name": backend.alias,
            "kind": "ollama",
            "registration_flag": format!("--ollama {}=<base-url>", backend.alias),
            "requires_api_key": false,
            "model_aliases": [format!("{}:<model>", backend.alias)],
            "capabilities": [
                "chat-messages",
                "response-format-json",
                "sampling",
                "seed-control",
                "stop-sequences",
                "usage-metadata",
            ],
        }));
    }

    for plugin_dir in plugin_dir {
        for backend in discover_plugin_backends(&plugin_dir)? {
            families.push(serde_json::json!({
                "name": backend.name,
                "kind": "plugin-command",
                "registration_flag": "--plugin-dir",
                "requires_api_key": false,
                "model_aliases": [format!("{}:<model>", backend.name)],
                "capabilities": [
                    "chat-messages",
                    "command-backend",
                    "usage-metadata",
                ],
            }));
        }
    }

    Ok(families)
}

fn print_model_runtimes(
    format: OutputFormat,
    backend: Vec<String>,
    ollama: Vec<String>,
    plugin_dir: Vec<PathBuf>,
) -> Result<()> {
    let models = model_runtime_views(backend, ollama, plugin_dir)?;
    match format {
        OutputFormat::Text => {
            for model in models {
                if let Some(name) = model.get("model").and_then(serde_json::Value::as_str) {
                    println!("{name}");
                }
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&models)?);
        }
    }

    Ok(())
}

fn model_runtime_views(
    backend: Vec<String>,
    ollama: Vec<String>,
    plugin_dir: Vec<PathBuf>,
) -> Result<Vec<serde_json::Value>> {
    let mut models = Vec::new();

    for family in builtin_backend_families() {
        for model in family.model_aliases {
            models.push(serde_json::json!({
                "model": model,
                "backend": family.name,
                "backend_kind": family.kind,
                "runtime": family.kind,
                "source": "built-in",
                "requires_api_key": family.requires_api_key,
                "registration_flag": family.registration_flag,
                "capabilities": family.capabilities,
            }));
        }
    }

    for backend in parse_alias_value_list(backend)? {
        models.push(serde_json::json!({
            "model": format!("{}:<model>", backend.alias),
            "backend": backend.alias,
            "backend_kind": "openai-compatible",
            "runtime": "remote-chat",
            "source": "cli",
            "requires_api_key": true,
            "registration_flag": format!("--backend {}=<base-url>", backend.alias),
            "capabilities": [
                "chat-messages",
                "response-format-json",
                "sampling",
                "seed-control",
                "stop-sequences",
                "streaming-inference",
                "usage-metadata",
            ],
        }));
    }

    for backend in parse_alias_value_list(ollama)? {
        models.push(serde_json::json!({
            "model": format!("{}:<model>", backend.alias),
            "backend": backend.alias,
            "backend_kind": "ollama",
            "runtime": "local-chat",
            "source": "cli",
            "requires_api_key": false,
            "registration_flag": format!("--ollama {}=<base-url>", backend.alias),
            "capabilities": [
                "chat-messages",
                "response-format-json",
                "sampling",
                "seed-control",
                "stop-sequences",
                "usage-metadata",
            ],
        }));
    }

    for plugin_dir in plugin_dir {
        for backend in discover_plugin_backends(&plugin_dir)? {
            models.push(serde_json::json!({
                "model": format!("{}:<model>", backend.name),
                "backend": backend.name,
                "backend_kind": "plugin-command",
                "runtime": "command",
                "source": "plugin",
                "requires_api_key": false,
                "registration_flag": "--plugin-dir",
                "capabilities": [
                    "chat-messages",
                    "command-backend",
                    "usage-metadata",
                ],
            }));
        }
    }

    Ok(models)
}

fn print_plugin_manifests(plugin_dir: &Path, format: OutputFormat) -> Result<()> {
    let manifests = discover_plugin_manifests(plugin_dir)?;
    match format {
        OutputFormat::Text => {
            for manifest in manifests {
                println!("{} {}", manifest.name, manifest.version);
                for capability in manifest.capabilities {
                    println!(
                        "  {} {} {}",
                        capability.kind, capability.name, capability.entrypoint
                    );
                }
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&manifests)?);
        }
    }

    Ok(())
}

fn validate_plugins(plugin_dir: &Path, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Text => {
            validate_plugin_manifests(plugin_dir)?;
            println!("ok");
        }
        OutputFormat::Json => {
            let report = validate_plugin_directory(plugin_dir)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            if !report.valid {
                anyhow::bail!("plugin validation failed");
            }
        }
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
    events: Option<PathBuf>,
    parallel: bool,
    plugin_dir: Vec<PathBuf>,
    stream_stage: Option<String>,
    backend: Vec<String>,
    ollama: Vec<String>,
    api_key_env: Vec<String>,
    api_key: Vec<String>,
) -> Result<()> {
    let (manifest, cwd) = load_pipeline_manifest(manifest_path, input_path, inline_graph)?;
    let engine = build_engine(backend, ollama, api_key_env, api_key, &plugin_dir)?;
    if stream_stage.is_some() && events.as_deref() == Some(Path::new("-")) {
        anyhow::bail!("stream-stage cannot write to stdout while events stream to stdout");
    }
    if events.as_deref() == Some(Path::new("-"))
        && manifest
            .outputs
            .values()
            .any(|output| output.path.as_str() == "-")
    {
        anyhow::bail!("events cannot stream to stdout while manifest outputs write to stdout");
    }
    if stream_stage.is_some()
        && manifest
            .outputs
            .values()
            .any(|output| output.path.as_str() == "-")
    {
        anyhow::bail!("stream-stage cannot write to stdout while manifest outputs write to stdout");
    }

    let stream_path = stream_stage.as_ref().map(|_| PathBuf::from("-"));
    let options = RunOptions {
        run_id: "cli-run".to_string(),
        trace_path: trace,
        event_path: events,
        scheduler: if parallel {
            SchedulerMode::Parallel
        } else {
            SchedulerMode::Sequential
        },
        plugin_dirs: plugin_dir,
        stream_stage,
        stream_path,
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
    plugin_dir: &[PathBuf],
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
    for plugin_dir in plugin_dir {
        for backend in discover_plugin_backends(plugin_dir)? {
            engine = engine.with_backend(
                backend.name.clone(),
                Arc::new(CommandBackend::new(backend.name, backend.entrypoint)),
            );
        }
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
