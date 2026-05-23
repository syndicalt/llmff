use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use llmff_core::backend::{
    builtin_backend_families, CommandBackend, MockBackend, OllamaBackend, OpenAiCompatibleBackend,
};
use llmff_core::engine::{Engine, RetryPolicy, RunOptions, SchedulerMode};
use llmff_core::graph::Graph;
use llmff_core::manifest::Manifest;
use llmff_core::plugin::{
    discover_plugin_backends, discover_plugin_manifests, validate_plugin_directory,
    PLUGIN_PROTOCOL_VERSION,
};
use llmff_core::stage::builtin_stage_metadata;
use sha2::{Digest, Sha256};

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
        #[arg(long = "max-concurrency")]
        max_concurrency: Option<usize>,
        #[arg(long = "timeout-ms")]
        timeout_ms: Option<u64>,
        #[arg(long = "retry-attempts")]
        retry_attempts: Option<usize>,
        #[arg(long = "retry-backoff-ms")]
        retry_backoff_ms: Option<u64>,
        #[arg(long = "checkpoint")]
        checkpoint: Option<PathBuf>,
        #[arg(long = "resume")]
        resume: Option<PathBuf>,
        #[arg(long = "replay-trace")]
        replay_trace: Option<PathBuf>,
        #[arg(long = "batch-input")]
        batch_input: Option<PathBuf>,
        #[arg(long = "batch-output-dir")]
        batch_output_dir: Option<PathBuf>,
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
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
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
    Report {
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
        #[arg(long = "backend")]
        backend: Vec<String>,
        #[arg(long = "ollama")]
        ollama: Vec<String>,
        #[arg(long = "api-key-env")]
        api_key_env: Vec<String>,
        #[arg(long = "api-key")]
        api_key: Vec<String>,
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
            max_concurrency,
            timeout_ms,
            retry_attempts,
            retry_backoff_ms,
            checkpoint,
            resume,
            replay_trace,
            batch_input,
            batch_output_dir,
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
                max_concurrency,
                timeout_ms,
                retry_attempts,
                retry_backoff_ms,
                checkpoint,
                resume,
                replay_trace,
                batch_input,
                batch_output_dir,
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
            format,
            plugin_dir,
            backend,
            ollama,
            api_key_env,
            api_key,
        } => {
            let loaded = load_pipeline_manifest(manifest, input, graph)?;
            let engine = build_engine(
                backend.clone(),
                ollama.clone(),
                api_key_env,
                api_key,
                &plugin_dir,
            )?;
            let graph =
                engine.validate_manifest_with_plugin_dirs(loaded.manifest.clone(), &plugin_dir)?;
            print_inspect_report(format, loaded, graph, &plugin_dir, &backend, &ollama)?;
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
        Command::Backends {
            command:
                BackendsCommand::Report {
                    format,
                    backend,
                    ollama,
                    api_key_env,
                    api_key,
                    plugin_dir,
                },
        } => print_backend_report(format, backend, ollama, api_key_env, api_key, plugin_dir)?,
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

#[derive(Debug)]
struct LoadedManifest {
    manifest: Manifest,
    cwd: PathBuf,
    source: ManifestSource,
}

#[derive(Debug)]
struct ManifestSource {
    kind: &'static str,
    path: Option<PathBuf>,
    content: String,
}

fn print_inspect_report(
    format: OutputFormat,
    loaded: LoadedManifest,
    graph: Graph,
    plugin_dirs: &[PathBuf],
    backend: &[String],
    ollama: &[String],
) -> Result<()> {
    match format {
        OutputFormat::Text => {
            println!("ok");
        }
        OutputFormat::Json => {
            let report = inspect_report(loaded, graph, plugin_dirs, backend, ollama)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
    }

    Ok(())
}

fn inspect_report(
    loaded: LoadedManifest,
    graph: Graph,
    plugin_dirs: &[PathBuf],
    backend: &[String],
    ollama: &[String],
) -> Result<serde_json::Value> {
    let stage_order = graph
        .stages()
        .iter()
        .map(|stage| stage.id.clone())
        .collect::<Vec<_>>();
    let stages = graph
        .stages()
        .iter()
        .map(inspect_stage_view)
        .collect::<Vec<_>>();
    let source_path = loaded
        .source
        .path
        .as_ref()
        .map(|path| path.to_string_lossy().to_string());
    let plugin_dirs = plugin_dirs
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let backend_registrations =
        inspect_backend_registrations(backend, ollama, plugin_dirs.iter().map(PathBuf::from))?;
    let plugin_manifests = inspect_plugin_manifests(plugin_dirs.iter().map(PathBuf::from))?;

    Ok(serde_json::json!({
        "format_version": 1,
        "compatibility": {
            "pipeline_manifest_schema": 1,
            "inspect_report_schema": 1,
            "inline_graph_syntax": 1,
            "plugin_protocol": PLUGIN_PROTOCOL_VERSION,
        },
        "manifest": {
            "version": loaded.manifest.version,
            "source": {
                "kind": loaded.source.kind,
                "path": source_path,
                "cwd": loaded.cwd.to_string_lossy(),
            },
            "hash": format!("sha256:{}", sha256_hex(loaded.source.content.as_bytes())),
        },
        "inputs": loaded.manifest.inputs,
        "outputs": loaded.manifest.outputs,
        "stage_order": stage_order,
        "stages": stages,
        "execution": {
            "scheduler": "sequential",
            "max_concurrency": null,
            "default_timeout_ms": null,
            "default_retry": {
                "attempts": 1,
                "backoff_ms": 0,
            },
            "checkpoint": {
                "enabled": false,
                "resume": false,
            },
            "stdout": {
                "events": false,
                "stream_stage": false,
                "manifest_outputs": loaded.manifest.outputs.values().any(|output| output.path == "-"),
            },
        },
        "backends": {
            "registrations": backend_registrations,
        },
        "plugins": {
            "directories": plugin_dirs,
            "protocol_version": PLUGIN_PROTOCOL_VERSION,
            "manifests": plugin_manifests,
        },
    }))
}

fn inspect_backend_registrations(
    backend: &[String],
    ollama: &[String],
    plugin_dirs: impl IntoIterator<Item = PathBuf>,
) -> Result<Vec<serde_json::Value>> {
    let mut registrations = builtin_backend_families()
        .iter()
        .map(|backend| {
            serde_json::json!({
                "name": backend.name,
                "kind": backend.kind,
                "source": "built-in",
                "registration_flag": backend.registration_flag,
                "base_url": null,
                "requires_api_key": backend.requires_api_key,
                "model_aliases": backend.model_aliases,
                "capabilities": backend.capabilities,
            })
        })
        .collect::<Vec<_>>();

    for backend in parse_alias_value_list(backend.to_vec())? {
        registrations.push(serde_json::json!({
            "name": backend.alias,
            "kind": "openai-compatible",
            "source": "cli",
            "registration_flag": format!("--backend {}=<base-url>", backend.alias),
            "base_url": backend.value,
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

    for backend in parse_alias_value_list(ollama.to_vec())? {
        registrations.push(serde_json::json!({
            "name": backend.alias,
            "kind": "ollama",
            "source": "cli",
            "registration_flag": format!("--ollama {}=<base-url>", backend.alias),
            "base_url": backend.value,
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

    for plugin_dir in plugin_dirs {
        for backend in discover_plugin_backends(&plugin_dir)? {
            registrations.push(serde_json::json!({
                "name": backend.name,
                "kind": "plugin-command",
                "source": "plugin",
                "registration_flag": "--plugin-dir",
                "base_url": null,
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

    Ok(registrations)
}

fn inspect_plugin_manifests(
    plugin_dirs: impl IntoIterator<Item = PathBuf>,
) -> Result<Vec<serde_json::Value>> {
    let mut manifests = Vec::new();
    for plugin_dir in plugin_dirs {
        for manifest in discover_plugin_manifests(&plugin_dir)? {
            manifests.push(serde_json::json!({
                "name": manifest.name,
                "version": manifest.version,
                "capabilities": manifest.capabilities,
            }));
        }
    }
    Ok(manifests)
}

fn inspect_stage_view(stage: &llmff_core::manifest::StageSpec) -> serde_json::Value {
    serde_json::json!({
        "id": stage.id,
        "op": stage.op,
        "input": stage.input,
        "from": stage.from,
        "model": stage.model.as_ref().map(|model| model_view(model)),
        "sampler": stage.sampler,
        "plugin": plugin_stage_view(&stage.op),
        "capability_constraints": stage_capability_constraints(&stage.op),
        "cache_policy": stage.cache_policy,
        "timeout_ms": stage.timeout_ms,
        "retry": stage.retry,
        "writes_stdout": stage.op == "write" && stage.path.as_deref() == Some("-"),
    })
}

fn stage_capability_constraints(op: &str) -> serde_json::Value {
    if let Some(metadata) = builtin_stage_metadata()
        .iter()
        .find(|metadata| metadata.name == op)
    {
        return serde_json::json!({
            "kind": metadata.kind,
            "required_fields": metadata.required_fields,
            "optional_fields": metadata.optional_fields,
            "capabilities": metadata.capabilities,
        });
    }

    if let Some(name) = op.strip_prefix("plugin:") {
        return serde_json::json!({
            "kind": "plugin-stage",
            "required_fields": ["from"],
            "optional_fields": [],
            "capabilities": ["plugin-stage"],
            "plugin": {
                "name": name,
            },
        });
    }

    serde_json::json!({
        "kind": "unknown",
        "required_fields": [],
        "optional_fields": [],
        "capabilities": [],
    })
}

fn model_view(model: &str) -> serde_json::Value {
    let (alias, provider_model) = model
        .split_once(':')
        .map(|(alias, provider_model)| (alias, provider_model))
        .unwrap_or((model, ""));
    serde_json::json!({
        "id": model,
        "alias": alias,
        "provider_model": provider_model,
    })
}

fn plugin_stage_view(op: &str) -> Option<serde_json::Value> {
    op.strip_prefix("plugin:").map(|name| {
        serde_json::json!({
            "kind": "stage",
            "name": name,
        })
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{digest:x}")
}

fn print_backend_report(
    format: OutputFormat,
    backend: Vec<String>,
    ollama: Vec<String>,
    api_key_env: Vec<String>,
    api_key: Vec<String>,
    plugin_dir: Vec<PathBuf>,
) -> Result<()> {
    let report = backend_report_views(backend, ollama, api_key_env, api_key, plugin_dir)?;
    match format {
        OutputFormat::Text => {
            for backend in report {
                let name = backend["name"].as_str().unwrap_or("unknown");
                let kind = backend["kind"].as_str().unwrap_or("unknown");
                println!("{name} ({kind})");
                for capability in ["json_mode", "streaming", "seed", "stop", "usage_metadata"] {
                    let supported = backend["capabilities"][capability]["supported"]
                        .as_bool()
                        .unwrap_or(false);
                    println!("  {capability}: {supported}");
                }
                if let Some(diagnostics) = backend["diagnostics"].as_array() {
                    for diagnostic in diagnostics {
                        if let Some(message) = diagnostic["message"].as_str() {
                            println!("  diagnostic: {message}");
                        }
                    }
                }
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
    }

    Ok(())
}

fn backend_report_views(
    backend: Vec<String>,
    ollama: Vec<String>,
    api_key_env: Vec<String>,
    api_key: Vec<String>,
    plugin_dir: Vec<PathBuf>,
) -> Result<Vec<serde_json::Value>> {
    let api_key_env = parse_alias_value_map(api_key_env)?;
    let api_key = parse_alias_value_map(api_key)?;
    let mut report = Vec::new();

    for family in builtin_backend_families() {
        let aliases = if family.model_aliases.is_empty() {
            vec![family.name.to_string()]
        } else {
            family
                .model_aliases
                .iter()
                .map(|alias| (*alias).to_string())
                .collect()
        };
        report.push(provider_report(
            family.name,
            family.kind,
            "built-in",
            None,
            family.requires_api_key,
            false,
            aliases,
        ));
    }

    for backend in parse_alias_value_list(backend)? {
        let key_configured =
            api_key.contains_key(&backend.alias) || api_key_env.contains_key(&backend.alias);
        report.push(provider_report(
            &backend.alias,
            "openai-compatible",
            "cli",
            Some(&backend.value),
            true,
            key_configured,
            vec![format!("{}:<model>", backend.alias)],
        ));
    }

    for backend in parse_alias_value_list(ollama)? {
        report.push(provider_report(
            &backend.alias,
            "ollama",
            "cli",
            Some(&backend.value),
            false,
            false,
            vec![format!("{}:<model>", backend.alias)],
        ));
    }

    for plugin_dir in plugin_dir {
        for backend in discover_plugin_backends(&plugin_dir)? {
            report.push(provider_report(
                &backend.name,
                "plugin-command",
                "plugin",
                None,
                false,
                false,
                vec![format!("{}:<model>", backend.name)],
            ));
        }
    }

    Ok(report)
}

fn provider_report(
    name: &str,
    kind: &str,
    source: &str,
    base_url: Option<&str>,
    requires_api_key: bool,
    api_key_configured: bool,
    model_aliases: Vec<String>,
) -> serde_json::Value {
    let mut diagnostics = Vec::new();
    if requires_api_key && !api_key_configured && source == "cli" {
        diagnostics.push(serde_json::json!({
            "severity": "warning",
            "code": "api_key_missing",
            "message": format!("backend `{name}` requires an API key; configure --api-key-env {name}=ENV_NAME or --api-key {name}=VALUE"),
        }));
    }
    if kind == "ollama" {
        diagnostics.push(serde_json::json!({
            "severity": "info",
            "code": "streaming_not_supported",
            "message": "llmff uses Ollama through the non-streaming chat path",
        }));
    }

    serde_json::json!({
        "name": name,
        "kind": kind,
        "source": source,
        "base_url": base_url,
        "requires_api_key": requires_api_key,
        "api_key_configured": api_key_configured,
        "model_aliases": model_aliases,
        "capabilities": provider_capabilities(kind),
        "diagnostics": diagnostics,
    })
}

fn provider_capabilities(kind: &str) -> serde_json::Value {
    match kind {
        "remote-chat" | "openai-compatible" => serde_json::json!({
            "json_mode": capability(true, "response_format json_object"),
            "streaming": capability(true, "server-sent chat completion chunks"),
            "seed": capability(true, "seed request field"),
            "stop": capability(true, "stop request field"),
            "usage_metadata": capability(true, "usage response field"),
        }),
        "local-chat" | "ollama" => serde_json::json!({
            "json_mode": capability(true, "format json"),
            "streaming": capability(false, "Ollama streaming is not wired through llmff yet"),
            "seed": capability(true, "options.seed request field"),
            "stop": capability(true, "options.stop request field"),
            "usage_metadata": capability(true, "prompt_eval_count and eval_count response fields"),
        }),
        "plugin-command" => serde_json::json!({
            "json_mode": capability(false, "plugin-specific"),
            "streaming": capability(false, "command backends are request-response"),
            "seed": capability(false, "plugin-specific"),
            "stop": capability(false, "plugin-specific"),
            "usage_metadata": capability(true, "optional usage response field"),
        }),
        _ => serde_json::json!({
            "json_mode": capability(false, "not applicable"),
            "streaming": capability(false, "not applicable"),
            "seed": capability(false, "not applicable"),
            "stop": capability(false, "not applicable"),
            "usage_metadata": capability(false, "not applicable"),
        }),
    }
}

fn capability(supported: bool, detail: &str) -> serde_json::Value {
    serde_json::json!({
        "supported": supported,
        "detail": detail,
    })
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
            let report = validate_plugin_directory(plugin_dir)?;
            if !report.valid {
                let messages = report
                    .diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.message.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                anyhow::bail!("{messages}");
            }
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
    max_concurrency: Option<usize>,
    timeout_ms: Option<u64>,
    retry_attempts: Option<usize>,
    retry_backoff_ms: Option<u64>,
    checkpoint: Option<PathBuf>,
    resume: Option<PathBuf>,
    replay_trace: Option<PathBuf>,
    batch_input: Option<PathBuf>,
    batch_output_dir: Option<PathBuf>,
    plugin_dir: Vec<PathBuf>,
    stream_stage: Option<String>,
    backend: Vec<String>,
    ollama: Vec<String>,
    api_key_env: Vec<String>,
    api_key: Vec<String>,
) -> Result<()> {
    let loaded = load_pipeline_manifest(manifest_path, input_path, inline_graph)?;
    let manifest = loaded.manifest;
    let cwd = loaded.cwd;
    let engine = build_engine(backend, ollama, api_key_env, api_key, &plugin_dir)?;
    if batch_input.is_some() || batch_output_dir.is_some() {
        return run_batch_pipeline(
            manifest,
            &cwd,
            &engine,
            batch_input,
            batch_output_dir,
            parallel,
            max_concurrency,
            timeout_ms,
            retry_attempts,
            retry_backoff_ms,
            plugin_dir,
        )
        .await;
    }
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
    if max_concurrency == Some(0) {
        anyhow::bail!("max-concurrency must be greater than 0");
    }
    if timeout_ms == Some(0) {
        anyhow::bail!("timeout-ms must be greater than 0");
    }
    if retry_attempts == Some(0) {
        anyhow::bail!("retry-attempts must be greater than 0");
    }

    let stream_path = stream_stage.as_ref().map(|_| PathBuf::from("-"));
    let default_retry = retry_attempts
        .map(|attempts| RetryPolicy {
            attempts,
            backoff_ms: retry_backoff_ms.unwrap_or(0),
        })
        .unwrap_or_default();
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
        max_concurrency,
        default_timeout_ms: timeout_ms,
        default_retry,
        checkpoint_path: checkpoint,
        resume_path: resume,
        replay_trace_path: replay_trace,
        ..RunOptions::default()
    };

    engine
        .run_manifest_with_options(manifest, &cwd, options)
        .await?;

    Ok(())
}

async fn run_batch_pipeline(
    manifest: Manifest,
    cwd: &Path,
    engine: &Engine,
    batch_input: Option<PathBuf>,
    batch_output_dir: Option<PathBuf>,
    parallel: bool,
    max_concurrency: Option<usize>,
    timeout_ms: Option<u64>,
    retry_attempts: Option<usize>,
    retry_backoff_ms: Option<u64>,
    plugin_dirs: Vec<PathBuf>,
) -> Result<()> {
    let batch_input =
        batch_input.ok_or_else(|| anyhow::anyhow!("batch mode requires --batch-input"))?;
    let batch_output_dir = batch_output_dir
        .ok_or_else(|| anyhow::anyhow!("batch mode requires --batch-output-dir"))?;
    if manifest.inputs.len() != 1 {
        anyhow::bail!("batch mode requires a manifest with exactly one input");
    }
    if manifest
        .outputs
        .values()
        .any(|output| output.path.as_str() == "-")
    {
        anyhow::bail!("batch mode requires file outputs, not stdout outputs");
    }
    if max_concurrency == Some(0) {
        anyhow::bail!("max-concurrency must be greater than 0");
    }
    if timeout_ms == Some(0) {
        anyhow::bail!("timeout-ms must be greater than 0");
    }
    if retry_attempts == Some(0) {
        anyhow::bail!("retry-attempts must be greater than 0");
    }

    std::fs::create_dir_all(batch_output_dir.join("inputs"))?;
    std::fs::create_dir_all(batch_output_dir.join("items"))?;
    let report_path = batch_output_dir.join("batch-report.jsonl");
    let mut report = std::io::BufWriter::new(std::fs::File::create(&report_path)?);
    let batch_source = std::fs::read_to_string(&batch_input)?;
    let input_name = manifest
        .inputs
        .keys()
        .next()
        .expect("input length checked above")
        .clone();
    let default_retry = retry_attempts
        .map(|attempts| RetryPolicy {
            attempts,
            backoff_ms: retry_backoff_ms.unwrap_or(0),
        })
        .unwrap_or_default();
    let mut failed = false;

    for (index, item) in batch_source.lines().enumerate() {
        let item_id = format!("{index:06}");
        let item_input_path = batch_output_dir
            .join("inputs")
            .join(format!("{item_id}.txt"));
        let item_output_dir = batch_output_dir.join("items").join(&item_id);
        std::fs::create_dir_all(&item_output_dir)?;
        std::fs::write(&item_input_path, item)?;

        let mut item_manifest = manifest.clone();
        item_manifest
            .inputs
            .get_mut(&input_name)
            .expect("input key should exist")
            .path = Some(item_input_path.to_string_lossy().into_owned());
        for output in item_manifest.outputs.values_mut() {
            let path = Path::new(&output.path);
            if path.is_absolute() {
                anyhow::bail!("batch mode requires relative output paths");
            }
            if path
                .components()
                .any(|component| matches!(component, Component::ParentDir))
            {
                anyhow::bail!("batch mode output paths cannot contain parent directory components");
            }
            output.path = item_output_dir.join(path).to_string_lossy().into_owned();
            if let Some(parent) = Path::new(&output.path).parent() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let options = RunOptions {
            run_id: format!("cli-batch-{item_id}"),
            scheduler: if parallel {
                SchedulerMode::Parallel
            } else {
                SchedulerMode::Sequential
            },
            plugin_dirs: plugin_dirs.clone(),
            max_concurrency,
            default_timeout_ms: timeout_ms,
            default_retry,
            ..RunOptions::default()
        };

        match engine
            .run_manifest_with_options(item_manifest, cwd, options)
            .await
        {
            Ok(_) => {
                writeln!(
                    report,
                    "{}",
                    serde_json::json!({"index": index, "status": "succeeded"})
                )?;
            }
            Err(error) => {
                failed = true;
                writeln!(
                    report,
                    "{}",
                    serde_json::json!({
                        "index": index,
                        "status": "failed",
                        "message": error.to_string()
                    })
                )?;
            }
        }
    }
    report.flush()?;

    if failed {
        anyhow::bail!(
            "one or more batch items failed; see {}",
            report_path.display()
        );
    }

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
) -> Result<LoadedManifest> {
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
            Ok(LoadedManifest {
                manifest,
                cwd,
                source: ManifestSource {
                    kind: "file",
                    path: Some(path),
                    content: source,
                },
            })
        }
        (None, Some(graph)) => {
            let input = input_path.map(|path| path.to_string_lossy().to_string());
            let manifest = Manifest::from_inline_graph(&graph, input)?;
            Ok(LoadedManifest {
                manifest,
                cwd: std::env::current_dir()?,
                source: ManifestSource {
                    kind: "inline_graph",
                    path: None,
                    content: graph,
                },
            })
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
