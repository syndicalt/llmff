use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use llmff_core::backend::{CommandBackend, MockBackend, OllamaBackend, OpenAiCompatibleBackend};
use llmff_core::engine::{Engine, RetryPolicy, RunOptions, SchedulerMode};
use llmff_core::graph::Graph;
use llmff_core::manifest::Manifest;
use llmff_core::plugin::{discover_plugin_backends, PLUGIN_PROTOCOL_VERSION};
use llmff_core::stage::builtin_stage_metadata;

mod batch;
mod doctor;
mod exit_codes;
mod plugins;
mod providers;
mod run_dir;

pub use batch::batch_exit_code;
use batch::{run_batch_pipeline, BatchPipelineRequest};
use doctor::{run_doctor, DoctorOptions};
pub use exit_codes::exit_code;
use plugins::{inspect_plugin_manifests, print_plugin_manifests, validate_plugins};
use providers::{
    inspect_backend_registrations, print_backend_families, print_backend_report,
    print_model_runtimes, print_stage_metadata,
};
use run_dir::{
    append_interrupted_run_event, finish_batch_run_dir_events, initialize_batch_run_dir_artifacts,
    interrupted_run_result_summary, manifest_bytes_for_failure, manifest_fingerprint,
    manifest_hash_for_interrupt, run_result_summary_for_error, run_result_summary_for_error_result,
    run_result_summary_for_llmff_error, sha256_hex, write_json_file, RunDirArtifacts,
};

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

#[derive(Debug, Clone)]
pub struct InterruptContext {
    run_dir: PathBuf,
    manifest_hash: String,
}

pub fn interrupt_context(cli: &Cli) -> Option<InterruptContext> {
    match &cli.command {
        Command::Run {
            manifest,
            graph,
            run_dir: Some(run_dir),
            ..
        } => Some(InterruptContext {
            run_dir: run_dir.clone(),
            manifest_hash: manifest_hash_for_interrupt(manifest.as_ref(), graph.as_ref()),
        }),
        _ => None,
    }
}

pub fn write_interrupted_run_result(context: &InterruptContext) -> Result<()> {
    let artifacts = RunDirArtifacts::new(&context.run_dir)?;
    append_interrupted_run_event(&artifacts)?;
    let result = interrupted_run_result_summary(&context.manifest_hash, &artifacts);
    write_json_file(&artifacts.result_path, &result)?;
    Ok(())
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
        #[arg(long = "run-dir")]
        run_dir: Option<PathBuf>,
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
        #[arg(long = "stream-stage")]
        stream_stage: Option<String>,
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
    Doctor {
        #[arg(long = "run-dir")]
        run_dir: Option<PathBuf>,
        #[arg(long = "plugin-dir")]
        plugin_dir: Vec<PathBuf>,
        #[arg(long = "backend")]
        backend: Vec<String>,
        #[arg(long = "api-key-env")]
        api_key_env: Vec<String>,
        #[arg(long = "release-manifest")]
        release_manifest: Option<PathBuf>,
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
pub(super) enum OutputFormat {
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
            run_dir,
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
            run_pipeline(RunPipelineArgs {
                manifest_path: manifest,
                input_path: input,
                inline_graph: graph,
                trace,
                events,
                run_dir,
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
            })
            .await?
        }
        Command::Inspect {
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
            stream_stage,
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
            let inspect_options = InspectExecutionOptions {
                trace,
                events,
                parallel,
                max_concurrency,
                timeout_ms,
                retry_attempts,
                retry_backoff_ms,
                checkpoint,
                resume,
                stream_stage,
            }
            .validate()?;
            inspect_options.validate_stdout_ownership(&loaded.manifest)?;
            print_inspect_report(
                format,
                loaded,
                graph,
                &plugin_dir,
                &backend,
                &ollama,
                inspect_options,
            )?;
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
        Command::Doctor {
            run_dir,
            plugin_dir,
            backend,
            api_key_env,
            release_manifest,
        } => run_doctor(DoctorOptions {
            run_dir,
            plugin_dir,
            backend,
            api_key_env,
            release_manifest,
        })?,
        Command::Trace { path } => summarize_trace(&path)?,
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct LoadedManifest {
    manifest: Manifest,
    cwd: PathBuf,
    source: ManifestSource,
}

#[derive(Debug, Clone)]
struct ManifestSource {
    kind: &'static str,
    path: Option<PathBuf>,
    content: String,
}

#[derive(Debug)]
struct InspectExecutionOptions {
    trace: Option<PathBuf>,
    events: Option<PathBuf>,
    parallel: bool,
    max_concurrency: Option<usize>,
    timeout_ms: Option<u64>,
    retry_attempts: Option<usize>,
    retry_backoff_ms: Option<u64>,
    checkpoint: Option<PathBuf>,
    resume: Option<PathBuf>,
    stream_stage: Option<String>,
}

impl InspectExecutionOptions {
    fn validate(self) -> Result<Self> {
        if self.max_concurrency == Some(0) {
            anyhow::bail!("max-concurrency must be greater than 0");
        }
        if self.timeout_ms == Some(0) {
            anyhow::bail!("timeout-ms must be greater than 0");
        }
        if self.retry_attempts == Some(0) {
            anyhow::bail!("retry-attempts must be greater than 0");
        }
        Ok(self)
    }

    fn default_retry(&self) -> RetryPolicy {
        self.retry_attempts
            .map(|attempts| RetryPolicy {
                attempts,
                backoff_ms: self.retry_backoff_ms.unwrap_or(0),
            })
            .unwrap_or_default()
    }

    fn validate_stdout_ownership(&self, manifest: &Manifest) -> Result<()> {
        let manifest_writes_stdout = manifest
            .outputs
            .values()
            .any(|output| output.path.as_str() == "-");
        if self.stream_stage.is_some() && self.events.as_deref() == Some(Path::new("-")) {
            anyhow::bail!("stream-stage cannot write to stdout while events stream to stdout");
        }
        if self.events.as_deref() == Some(Path::new("-")) && manifest_writes_stdout {
            anyhow::bail!("events cannot stream to stdout while manifest outputs write to stdout");
        }
        if self.stream_stage.is_some() && manifest_writes_stdout {
            anyhow::bail!(
                "stream-stage cannot write to stdout while manifest outputs write to stdout"
            );
        }
        Ok(())
    }
}

fn print_inspect_report(
    format: OutputFormat,
    loaded: LoadedManifest,
    graph: Graph,
    plugin_dirs: &[PathBuf],
    backend: &[String],
    ollama: &[String],
    options: InspectExecutionOptions,
) -> Result<()> {
    match format {
        OutputFormat::Text => {
            println!("ok");
        }
        OutputFormat::Json => {
            let report = inspect_report(loaded, graph, plugin_dirs, backend, ollama, &options)?;
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
    options: &InspectExecutionOptions,
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
    let default_retry = options.default_retry();

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
            "hash": format!("sha256:{}", manifest_fingerprint(&loaded.manifest)?),
        },
        "inputs": loaded.manifest.inputs,
        "outputs": loaded.manifest.outputs,
        "stage_order": stage_order,
        "stages": stages,
        "execution": {
            "scheduler": if options.parallel { "parallel" } else { "sequential" },
            "max_concurrency": options.max_concurrency,
            "default_timeout_ms": options.timeout_ms,
            "default_retry": {
                "attempts": default_retry.attempts,
                "backoff_ms": default_retry.backoff_ms,
            },
            "checkpoint": {
                "enabled": options.checkpoint.is_some(),
                "resume": options.resume.is_some(),
                "path": options.checkpoint.as_ref().map(|path| path.to_string_lossy().to_string()),
                "resume_path": options.resume.as_ref().map(|path| path.to_string_lossy().to_string()),
            },
            "stdout": {
                "events": options.events.as_deref() == Some(Path::new("-")),
                "stream_stage": options.stream_stage.is_some(),
                "manifest_outputs": loaded.manifest.outputs.values().any(|output| output.path == "-"),
            },
            "artifacts": {
                "trace": options.trace.as_ref().map(|path| path.to_string_lossy().to_string()),
                "events": options.events.as_ref().map(|path| path.to_string_lossy().to_string()),
                "stream_stage": options.stream_stage.as_ref(),
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
        "loop": inspect_loop_metadata(stage),
        "map": inspect_map_metadata(stage),
        "cache_policy": stage.cache_policy,
        "timeout_ms": stage.timeout_ms,
        "retry": stage.retry,
        "writes_stdout": stage.op == "write" && stage.path.as_deref() == Some("-"),
    })
}

fn inspect_loop_metadata(stage: &llmff_core::manifest::StageSpec) -> serde_json::Value {
    if stage.op != "loop" {
        return serde_json::Value::Null;
    }

    let max_iterations = stage.max_iterations.unwrap_or(0);
    let body_stage_count = stage.body.len();
    serde_json::json!({
        "max_iterations": max_iterations,
        "body_stage_count": body_stage_count,
        "max_expanded_stage_count": max_iterations * body_stage_count,
        "break_on": stage.break_on,
        "final": stage.final_output,
        "retain_iterations": inspect_loop_retention(stage),
        "on_iteration_error": stage.on_iteration_error.as_deref().unwrap_or("fail")
    })
}

fn inspect_map_metadata(stage: &llmff_core::manifest::StageSpec) -> serde_json::Value {
    if stage.op != "map" {
        return serde_json::Value::Null;
    }

    let max_items = stage.max_items.unwrap_or(0);
    let body_stage_count = stage.body.len();
    serde_json::json!({
        "items_from": stage.items_from,
        "max_items": max_items,
        "body_stage_count": body_stage_count,
        "max_expanded_stage_count": max_items * body_stage_count,
        "final": stage.final_output,
        "parallel": stage.parallel.unwrap_or(false),
        "max_concurrency": stage.max_concurrency
    })
}

fn inspect_loop_retention(stage: &llmff_core::manifest::StageSpec) -> serde_json::Value {
    match &stage.retain_iterations {
        Some(retention) => serde_json::to_value(retention).unwrap_or(serde_json::Value::Null),
        None => serde_json::Value::String("none".to_string()),
    }
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
    let (alias, provider_model) = model.split_once(':').unwrap_or((model, ""));
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

struct RunPipelineArgs {
    manifest_path: Option<PathBuf>,
    input_path: Option<PathBuf>,
    inline_graph: Option<String>,
    trace: Option<PathBuf>,
    events: Option<PathBuf>,
    run_dir: Option<PathBuf>,
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
}

async fn run_pipeline(args: RunPipelineArgs) -> Result<()> {
    let RunPipelineArgs {
        manifest_path,
        input_path,
        inline_graph,
        trace,
        events,
        run_dir,
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
    } = args;

    if run_dir.is_some() && (trace.is_some() || events.is_some() || checkpoint.is_some()) {
        anyhow::bail!(
            "--run-dir owns trace, events, and checkpoint paths; do not combine it with --trace, --events, or --checkpoint"
        );
    }
    let batch_mode = batch_input.is_some() || batch_output_dir.is_some();
    if batch_mode {
        let unsupported_explicit_metadata =
            run_dir.is_none() && (trace.is_some() || events.is_some() || checkpoint.is_some());
        let unsupported_run_control =
            resume.is_some() || replay_trace.is_some() || stream_stage.is_some();
        if unsupported_explicit_metadata || unsupported_run_control {
            anyhow::bail!(
                "batch mode does not support explicit trace, events, checkpoint, resume, replay-trace, or stream-stage flags; use --run-dir for batch supervisor metadata"
            );
        }
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

    let run_dir_artifacts = run_dir
        .as_ref()
        .map(|path| RunDirArtifacts::new(path))
        .transpose()?;
    let failure_manifest_bytes =
        manifest_bytes_for_failure(manifest_path.as_ref(), inline_graph.as_ref());
    let loaded = match load_pipeline_manifest(manifest_path, input_path, inline_graph) {
        Ok(loaded) => loaded,
        Err(error) => {
            if let Some(artifacts) = run_dir_artifacts.as_ref() {
                let failure_manifest_hash = sha256_hex(&failure_manifest_bytes);
                let summary =
                    run_result_summary_for_error(&failure_manifest_hash, artifacts, &error);
                write_json_file(&artifacts.result_path, &summary)?;
            }
            return Err(error);
        }
    };
    let manifest = loaded.manifest.clone();
    let manifest_hash = manifest_fingerprint(&manifest)?;
    let cwd = loaded.cwd.clone();
    let backend_args = backend.clone();
    let ollama_args = ollama.clone();
    let engine = match build_engine(backend, ollama, api_key_env, api_key, &plugin_dir) {
        Ok(engine) => engine,
        Err(error) => {
            if let Some(artifacts) = run_dir_artifacts.as_ref() {
                let summary = run_result_summary_for_error(&manifest_hash, artifacts, &error);
                write_json_file(&artifacts.result_path, &summary)?;
            }
            return Err(error);
        }
    };
    let trace = trace.or_else(|| {
        run_dir_artifacts
            .as_ref()
            .map(|artifacts| artifacts.trace_path.clone())
    });
    let events = events.or_else(|| {
        run_dir_artifacts
            .as_ref()
            .map(|artifacts| artifacts.events_path.clone())
    });
    let checkpoint = checkpoint.or_else(|| {
        run_dir_artifacts
            .as_ref()
            .map(|artifacts| artifacts.checkpoint_path.clone())
    });
    if batch_mode {
        if let Some(artifacts) = run_dir_artifacts.as_ref() {
            let graph = match engine
                .validate_manifest_with_plugin_dirs(manifest.clone(), &plugin_dir)
            {
                Ok(graph) => graph,
                Err(error) => {
                    let summary =
                        run_result_summary_for_llmff_error(&manifest_hash, artifacts, Some(&error));
                    write_json_file(&artifacts.result_path, &summary)?;
                    return Err(error.into());
                }
            };
            let inspect_options = InspectExecutionOptions {
                trace: trace.clone(),
                events: events.clone(),
                parallel,
                max_concurrency,
                timeout_ms,
                retry_attempts,
                retry_backoff_ms,
                checkpoint: checkpoint.clone(),
                resume: resume.clone(),
                stream_stage: stream_stage.clone(),
            }
            .validate()?;
            inspect_options.validate_stdout_ownership(&manifest)?;
            let inspect = inspect_report(
                loaded.clone(),
                graph,
                &plugin_dir,
                &backend_args,
                &ollama_args,
                &inspect_options,
            )?;
            write_json_file(&artifacts.inspect_path, &inspect)?;
            initialize_batch_run_dir_artifacts(artifacts, &manifest)?;
        }

        let result = run_batch_pipeline(BatchPipelineRequest {
            manifest,
            cwd: &cwd,
            engine: &engine,
            batch_input,
            batch_output_dir,
            parallel,
            max_concurrency,
            timeout_ms,
            retry_attempts,
            retry_backoff_ms,
            plugin_dirs: plugin_dir,
            run_dir_artifacts: run_dir_artifacts.as_ref(),
        })
        .await;

        if let Some(artifacts) = run_dir_artifacts.as_ref() {
            finish_batch_run_dir_events(artifacts, result.as_ref().err())?;
            let summary = run_result_summary_for_error_result(
                &manifest_hash,
                artifacts,
                result.as_ref().err(),
            );
            write_json_file(&artifacts.result_path, &summary)?;
        }

        result?;

        return Ok(());
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
        let error = anyhow::anyhow!(
            "stream-stage cannot write to stdout while manifest outputs write to stdout"
        );
        if let Some(artifacts) = run_dir_artifacts.as_ref() {
            let summary = run_result_summary_for_error(&manifest_hash, artifacts, &error);
            write_json_file(&artifacts.result_path, &summary)?;
        }
        return Err(error);
    }
    if let Some(artifacts) = run_dir_artifacts.as_ref() {
        let graph = match engine.validate_manifest_with_plugin_dirs(manifest.clone(), &plugin_dir) {
            Ok(graph) => graph,
            Err(error) => {
                let summary =
                    run_result_summary_for_llmff_error(&manifest_hash, artifacts, Some(&error));
                write_json_file(&artifacts.result_path, &summary)?;
                return Err(error.into());
            }
        };
        let inspect_options = InspectExecutionOptions {
            trace: trace.clone(),
            events: events.clone(),
            parallel,
            max_concurrency,
            timeout_ms,
            retry_attempts,
            retry_backoff_ms,
            checkpoint: checkpoint.clone(),
            resume: resume.clone(),
            stream_stage: stream_stage.clone(),
        }
        .validate()?;
        inspect_options.validate_stdout_ownership(&manifest)?;
        let inspect = inspect_report(
            loaded.clone(),
            graph,
            &plugin_dir,
            &backend_args,
            &ollama_args,
            &inspect_options,
        )?;
        write_json_file(&artifacts.inspect_path, &inspect)?;
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
    };

    let result = engine
        .run_manifest_with_options(manifest, &cwd, options)
        .await;

    if let Some(artifacts) = run_dir_artifacts.as_ref() {
        let summary = match &result {
            Ok(_) => run_result_summary_for_llmff_error(&manifest_hash, artifacts, None),
            Err(error) => {
                run_result_summary_for_llmff_error(&manifest_hash, artifacts, Some(error))
            }
        };
        write_json_file(&artifacts.result_path, &summary)?;
    }

    result?;

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
