use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt as _;
use tokio::process::Command;

use crate::backend::{Backend, InferRequest, InferResponse, UsageMetadata};
use crate::error::LlmffError;
use crate::graph::Graph;
use crate::manifest::{LoopBreakSpec, Manifest, RetrySpec, StageSpec};
use crate::plugin::{
    discover_plugin_samplers, discover_plugin_stages, discover_plugin_tool_transports,
    PluginSampler, PluginStage, PluginToolTransport,
};
use crate::stage::execute_deterministic_stage;
use crate::trace::{TraceEvent, TraceWriter};
use crate::value::{Message, StageStatus, Value};

mod checkpoint;
mod scheduler;
mod streaming;
mod trace_failure;

use checkpoint::{manifest_fingerprint, read_checkpoint, validate_replay_trace};
use scheduler::{run_stages_in_parallel, run_stages_sequentially};
use streaming::{create_stage_stream_writer, StageStreamWriter};
use trace_failure::{create_trace_writers, write_run_failed, write_trace};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunReport {
    pub final_status: RunStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerMode {
    Sequential,
    Parallel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    pub attempts: usize,
    pub backoff_ms: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            attempts: 1,
            backoff_ms: 0,
        }
    }
}

pub(super) struct StageOutcome {
    status: StageStatus,
    usage: Option<UsageMetadata>,
    cache_hit: Option<bool>,
    cache_path: Option<String>,
    attempts: Option<usize>,
    stream_written: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LoopTraceContext<'a> {
    loop_id: &'a str,
    iteration: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
enum LoopStopReason {
    BreakCondition,
    MaxIterations,
    Error,
}

struct InferAttemptResult {
    response: InferResponse,
    attempts: usize,
}

struct ToolAttemptResult {
    status: StageStatus,
    attempts: usize,
}

pub(super) struct PluginExecutionContext<'a> {
    tool_transports: &'a BTreeMap<String, PluginToolTransport>,
    stages: &'a BTreeMap<String, PluginStage>,
    samplers: &'a BTreeMap<String, PluginSampler>,
}

pub(super) struct ExecutionContext<'a> {
    manifest: &'a Manifest,
    pub(super) graph: &'a Graph,
    cwd: &'a Path,
    pub(super) run_id: &'a str,
    plugins: PluginExecutionContext<'a>,
    pub(super) options: &'a RunOptions,
    pub(super) manifest_hash: &'a str,
}

impl StageOutcome {
    fn without_usage(status: StageStatus) -> Self {
        Self {
            status,
            usage: None,
            cache_hit: None,
            cache_path: None,
            attempts: None,
            stream_written: false,
        }
    }

    fn with_usage_attempts(
        status: StageStatus,
        usage: Option<UsageMetadata>,
        attempts: usize,
    ) -> Self {
        Self {
            status,
            usage,
            cache_hit: None,
            cache_path: None,
            attempts: (attempts > 1).then_some(attempts),
            stream_written: false,
        }
    }

    fn with_streamed_usage(status: StageStatus, usage: Option<UsageMetadata>) -> Self {
        Self {
            status,
            usage,
            cache_hit: None,
            cache_path: None,
            attempts: None,
            stream_written: true,
        }
    }

    fn with_cache(status: StageStatus, cache_hit: bool, cache_path: String) -> Self {
        Self {
            status,
            usage: None,
            cache_hit: Some(cache_hit),
            cache_path: Some(cache_path),
            attempts: None,
            stream_written: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOptions {
    pub run_id: String,
    pub trace_path: Option<PathBuf>,
    pub event_path: Option<PathBuf>,
    pub scheduler: SchedulerMode,
    pub plugin_dirs: Vec<PathBuf>,
    pub stream_stage: Option<String>,
    pub stream_path: Option<PathBuf>,
    pub max_concurrency: Option<usize>,
    pub default_timeout_ms: Option<u64>,
    pub default_retry: RetryPolicy,
    pub checkpoint_path: Option<PathBuf>,
    pub resume_path: Option<PathBuf>,
    pub replay_trace_path: Option<PathBuf>,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            run_id: "local-run".to_string(),
            trace_path: None,
            event_path: None,
            scheduler: SchedulerMode::Sequential,
            plugin_dirs: Vec::new(),
            stream_stage: None,
            stream_path: None,
            max_concurrency: None,
            default_timeout_ms: None,
            default_retry: RetryPolicy::default(),
            checkpoint_path: None,
            resume_path: None,
            replay_trace_path: None,
        }
    }
}

#[derive(Default)]
pub struct Engine {
    backends: BTreeMap<String, Arc<dyn Backend>>,
}

impl Engine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_backend(mut self, model: impl Into<String>, backend: Arc<dyn Backend>) -> Self {
        self.backends.insert(model.into(), backend);
        self
    }

    pub fn validate_manifest(&self, manifest: Manifest) -> Result<Graph, LlmffError> {
        self.validate_manifest_with_plugins(manifest, &BTreeMap::new(), &BTreeMap::new())
    }

    pub fn validate_manifest_with_plugin_dirs(
        &self,
        manifest: Manifest,
        plugin_dirs: &[PathBuf],
    ) -> Result<Graph, LlmffError> {
        let plugin_stages = load_plugin_stages(plugin_dirs)?;
        let plugin_samplers = load_plugin_samplers(plugin_dirs)?;
        self.validate_manifest_with_plugins(manifest, &plugin_stages, &plugin_samplers)
    }

    fn validate_manifest_with_plugins(
        &self,
        manifest: Manifest,
        plugin_stages: &BTreeMap<String, PluginStage>,
        plugin_samplers: &BTreeMap<String, PluginSampler>,
    ) -> Result<Graph, LlmffError> {
        validate_input_formats(&manifest)?;
        let graph = Graph::from_manifest(manifest.clone())?;
        for stage in graph.stages() {
            self.validate_stage(stage, plugin_stages, plugin_samplers)?;
        }
        validate_stage_types(&graph, &manifest)?;

        Ok(graph)
    }

    pub async fn run_manifest(
        &self,
        manifest: Manifest,
        cwd: &Path,
    ) -> Result<RunReport, LlmffError> {
        self.run_manifest_with_options(manifest, cwd, RunOptions::default())
            .await
    }

    pub async fn run_manifest_with_options(
        &self,
        manifest: Manifest,
        cwd: &Path,
        options: RunOptions,
    ) -> Result<RunReport, LlmffError> {
        let run_id = options.run_id.clone();
        let mut trace = create_trace_writers(&options)?;
        match self
            .run_manifest_inner(manifest, cwd, options, &mut trace)
            .await
        {
            Ok(report) => Ok(report),
            Err(error) => {
                write_run_failed(&mut trace, &run_id, &error)?;
                Err(error)
            }
        }
    }

    async fn run_manifest_inner(
        &self,
        manifest: Manifest,
        cwd: &Path,
        options: RunOptions,
        trace: &mut Vec<TraceWriter>,
    ) -> Result<RunReport, LlmffError> {
        let plugin_tool_transports = load_plugin_tool_transports(&options.plugin_dirs)?;
        let plugin_stages = load_plugin_stages(&options.plugin_dirs)?;
        let plugin_samplers = load_plugin_samplers(&options.plugin_dirs)?;
        if options.max_concurrency == Some(0) {
            return Err(LlmffError::Config(
                "max_concurrency must be greater than 0".to_string(),
            ));
        }
        if let Some(path) = &options.replay_trace_path {
            validate_replay_trace(path, options.resume_path.is_some())?;
        }
        let graph = self.validate_manifest_with_plugins(
            manifest.clone(),
            &plugin_stages,
            &plugin_samplers,
        )?;
        if options.stream_stage.is_some() && options.scheduler == SchedulerMode::Parallel {
            return Err(LlmffError::Config(
                "stream-stage cannot be used with the parallel scheduler".to_string(),
            ));
        }
        let mut stream_writer = create_stage_stream_writer(&options, &graph, cwd)?;
        let manifest_hash = manifest_fingerprint(&manifest)?;
        let mut statuses = if let Some(path) = &options.resume_path {
            read_checkpoint(path, &manifest_hash)?
        } else {
            BTreeMap::new()
        };
        let context = ExecutionContext {
            manifest: &manifest,
            graph: &graph,
            cwd,
            run_id: &options.run_id,
            plugins: PluginExecutionContext {
                tool_transports: &plugin_tool_transports,
                stages: &plugin_stages,
                samplers: &plugin_samplers,
            },
            options: &options,
            manifest_hash: &manifest_hash,
        };

        write_trace(
            trace,
            TraceEvent {
                run_id: options.run_id.clone(),
                event: "run_started".to_string(),
                stage_id: None,
                loop_id: None,
                loop_iteration: None,
                loop_stage_id: None,
                op: None,
                status: None,
                timestamp_ms: timestamp_ms(),
                duration_ms: None,
                attempts: None,
                model: None,
                backend: None,
                provider_model: None,
                validation_errors: None,
                tool_kind: None,
                tool_target: None,
                output_path: None,
                prompt_tokens: None,
                completion_tokens: None,
                total_tokens: None,
                cache_hit: None,
                cache_path: None,
                failure_kind: None,
                failure_message: None,
            },
        )?;

        match options.scheduler {
            SchedulerMode::Sequential => {
                run_stages_sequentially(
                    self,
                    &context,
                    &mut statuses,
                    trace,
                    stream_writer.as_mut(),
                )
                .await?;
            }
            SchedulerMode::Parallel => {
                run_stages_in_parallel(self, &context, &mut statuses, trace).await?;
            }
        }

        for output in manifest.outputs.values() {
            let status = statuses
                .get(&output.from)
                .ok_or_else(|| LlmffError::StageExecution {
                    stage_id: output.from.clone(),
                    message: "output references missing stage".to_string(),
                })?;
            let value = match status {
                StageStatus::Success(value) => value,
                StageStatus::Invalid { errors, .. } => {
                    return Err(LlmffError::StageExecution {
                        stage_id: output.from.clone(),
                        message: format!("output references invalid stage: {}", errors.join("; ")),
                    });
                }
                StageStatus::Skipped => {
                    return Err(LlmffError::StageExecution {
                        stage_id: output.from.clone(),
                        message: "output references skipped stage".to_string(),
                    });
                }
            };
            write_output(cwd, &output.path, &serialize_value(value)?)?;
        }

        let report = RunReport {
            final_status: RunStatus::Succeeded,
        };
        write_trace(
            trace,
            TraceEvent {
                run_id: options.run_id,
                event: "run_finished".to_string(),
                stage_id: None,
                loop_id: None,
                loop_iteration: None,
                loop_stage_id: None,
                op: None,
                status: Some("succeeded".to_string()),
                timestamp_ms: timestamp_ms(),
                duration_ms: None,
                attempts: None,
                model: None,
                backend: None,
                provider_model: None,
                validation_errors: None,
                tool_kind: None,
                tool_target: None,
                output_path: None,
                prompt_tokens: None,
                completion_tokens: None,
                total_tokens: None,
                cache_hit: None,
                cache_path: None,
                failure_kind: None,
                failure_message: None,
            },
        )?;

        Ok(report)
    }

    fn start_stage_trace(
        &self,
        trace: &mut Vec<TraceWriter>,
        run_id: &str,
        stage: &StageSpec,
    ) -> Result<Instant, LlmffError> {
        self.start_stage_trace_with_loop(trace, run_id, stage, &stage.id, None)
    }

    fn start_stage_trace_with_loop(
        &self,
        trace: &mut Vec<TraceWriter>,
        run_id: &str,
        stage: &StageSpec,
        loop_stage_id: &str,
        loop_context: Option<LoopTraceContext<'_>>,
    ) -> Result<Instant, LlmffError> {
        let started = Instant::now();
        let loop_id = loop_context.map(|context| context.loop_id.to_string());
        let loop_iteration = loop_context.map(|context| context.iteration);
        write_trace(
            trace,
            TraceEvent {
                run_id: run_id.to_string(),
                event: "stage_started".to_string(),
                stage_id: Some(stage.id.clone()),
                loop_id,
                loop_iteration,
                loop_stage_id: loop_context.map(|_| loop_stage_id.to_string()),
                op: Some(stage.op.clone()),
                status: None,
                timestamp_ms: timestamp_ms(),
                duration_ms: None,
                attempts: None,
                model: None,
                backend: None,
                provider_model: None,
                validation_errors: None,
                tool_kind: None,
                tool_target: None,
                output_path: None,
                prompt_tokens: None,
                completion_tokens: None,
                total_tokens: None,
                cache_hit: None,
                cache_path: None,
                failure_kind: None,
                failure_message: None,
            },
        )?;
        Ok(started)
    }

    fn finish_stage_trace(
        &self,
        trace: &mut Vec<TraceWriter>,
        run_id: &str,
        stage: &StageSpec,
        stage_started: Instant,
        outcome: StageOutcome,
        statuses: &mut BTreeMap<String, StageStatus>,
    ) -> Result<(), LlmffError> {
        self.finish_stage_trace_with_loop(
            trace,
            run_id,
            stage,
            &stage.id,
            &stage.id,
            stage_started,
            outcome,
            statuses,
            None,
        )
    }

    fn finish_stage_trace_with_loop(
        &self,
        trace: &mut Vec<TraceWriter>,
        run_id: &str,
        trace_stage: &StageSpec,
        status_stage_id: &str,
        loop_stage_id: &str,
        stage_started: Instant,
        outcome: StageOutcome,
        statuses: &mut BTreeMap<String, StageStatus>,
        loop_context: Option<LoopTraceContext<'_>>,
    ) -> Result<(), LlmffError> {
        let status = outcome.status;
        let status_name = status_name(&status).to_string();
        let metadata = self.trace_metadata(trace_stage, &status, outcome.usage.as_ref());
        let cache_hit = outcome.cache_hit;
        let cache_path = outcome.cache_path;
        let attempts = outcome.attempts;
        let loop_id = loop_context.map(|context| context.loop_id.to_string());
        let loop_iteration = loop_context.map(|context| context.iteration);
        statuses.insert(status_stage_id.to_string(), status);
        write_trace(
            trace,
            TraceEvent {
                run_id: run_id.to_string(),
                event: "stage_finished".to_string(),
                stage_id: Some(trace_stage.id.clone()),
                loop_id,
                loop_iteration,
                loop_stage_id: loop_context.map(|_| loop_stage_id.to_string()),
                op: Some(trace_stage.op.clone()),
                status: Some(status_name),
                timestamp_ms: timestamp_ms(),
                duration_ms: Some(stage_started.elapsed().as_millis()),
                attempts,
                model: metadata.model,
                backend: metadata.backend,
                provider_model: metadata.provider_model,
                validation_errors: metadata.validation_errors,
                tool_kind: metadata.tool_kind,
                tool_target: metadata.tool_target,
                output_path: metadata.output_path,
                prompt_tokens: metadata.prompt_tokens,
                completion_tokens: metadata.completion_tokens,
                total_tokens: metadata.total_tokens,
                cache_hit,
                cache_path,
                failure_kind: None,
                failure_message: None,
            },
        )
    }

    fn finish_stage_trace_error_with_loop(
        &self,
        trace: &mut Vec<TraceWriter>,
        run_id: &str,
        trace_stage: &StageSpec,
        loop_stage_id: &str,
        stage_started: Instant,
        loop_context: LoopTraceContext<'_>,
    ) -> Result<(), LlmffError> {
        write_trace(
            trace,
            TraceEvent {
                run_id: run_id.to_string(),
                event: "stage_finished".to_string(),
                stage_id: Some(trace_stage.id.clone()),
                loop_id: Some(loop_context.loop_id.to_string()),
                loop_iteration: Some(loop_context.iteration),
                loop_stage_id: Some(loop_stage_id.to_string()),
                op: Some(trace_stage.op.clone()),
                status: Some("error".to_string()),
                timestamp_ms: timestamp_ms(),
                duration_ms: Some(stage_started.elapsed().as_millis()),
                attempts: None,
                model: None,
                backend: None,
                provider_model: None,
                validation_errors: None,
                tool_kind: None,
                tool_target: None,
                output_path: None,
                prompt_tokens: None,
                completion_tokens: None,
                total_tokens: None,
                cache_hit: None,
                cache_path: None,
                failure_kind: None,
                failure_message: None,
            },
        )
    }

    fn validate_stage(
        &self,
        stage: &StageSpec,
        plugin_stages: &BTreeMap<String, PluginStage>,
        plugin_samplers: &BTreeMap<String, PluginSampler>,
    ) -> Result<(), LlmffError> {
        validate_when_condition(stage)?;
        validate_sampling_parameters(stage)?;

        if let Some(plugin_stage_name) = plugin_stage_name(&stage.op) {
            reject_sampler_on_non_model_stage(stage)?;
            require_stage_field(stage, stage.from.as_deref(), "plugin stage requires from")?;
            return if plugin_stages.contains_key(plugin_stage_name) {
                Ok(())
            } else {
                Err(stage_validation_error(
                    stage,
                    format!("unknown plugin stage `{plugin_stage_name}`"),
                ))
            };
        }

        if !matches!(stage.op.as_str(), "infer" | "repair") {
            reject_sampler_on_non_model_stage(stage)?;
        }

        match stage.op.as_str() {
            "load" => {
                require_stage_field(stage, stage.input.as_deref(), "load requires input")?;
                Ok(())
            }
            "infer" => {
                require_stage_field(stage, stage.from.as_deref(), "infer requires from")?;
                validate_sampler_reference(stage, plugin_samplers)?;
                let model =
                    require_stage_field(stage, stage.model.as_deref(), "infer requires model")?;
                self.backend_for_model(model).map(|_| ())
            }
            "validate_json" => {
                require_stage_field(stage, stage.from.as_deref(), "validate_json requires from")?;
                if stage.schema.is_none() && stage.schema_path.is_none() {
                    return Err(stage_validation_error(
                        stage,
                        "validate_json requires schema or schema_path",
                    ));
                }
                Ok(())
            }
            "system" => {
                require_stage_field(stage, stage.from.as_deref(), "system requires from")?;
                Ok(())
            }
            "template" => {
                require_stage_field(stage, stage.from.as_deref(), "template requires from")?;
                require_stage_field(stage, stage.path.as_deref(), "template requires path")?;
                Ok(())
            }
            "retrieve" => {
                require_stage_field(stage, stage.from.as_deref(), "retrieve requires from")?;
                if stage.documents.is_empty() && !is_command_retrieval_strategy(stage) {
                    return Err(stage_validation_error(stage, "retrieve requires documents"));
                }
                if is_command_retrieval_strategy(stage) && stage.command.is_none() {
                    return Err(stage_validation_error(
                        stage,
                        "retrieve command strategy requires command",
                    ));
                }
                if stage.index.is_some() && stage.strategy.as_deref() != Some("embedding") {
                    return Err(stage_validation_error(
                        stage,
                        "retrieve index requires embedding strategy",
                    ));
                }
                if let Some(0) = stage.top_k {
                    return Err(stage_validation_error(
                        stage,
                        "retrieve top_k must be greater than 0",
                    ));
                }
                validate_retrieve_strategy(stage)?;
                Ok(())
            }
            "rerank" => {
                require_stage_field(stage, stage.from.as_deref(), "rerank requires from")?;
                if is_command_retrieval_strategy(stage) && stage.command.is_none() {
                    return Err(stage_validation_error(
                        stage,
                        "rerank command strategy requires command",
                    ));
                }
                if let Some(0) = stage.top_k {
                    return Err(stage_validation_error(
                        stage,
                        "rerank top_k must be greater than 0",
                    ));
                }
                validate_rerank_strategy(stage)?;
                Ok(())
            }
            "cache" => {
                require_stage_field(stage, stage.from.as_deref(), "cache requires from")?;
                Ok(())
            }
            "repair" => {
                require_stage_field(stage, stage.from.as_deref(), "repair requires from")?;
                validate_sampler_reference(stage, plugin_samplers)?;
                let model =
                    require_stage_field(stage, stage.model.as_deref(), "repair requires model")?;
                self.backend_for_model(model).map(|_| ())
            }
            "route" => {
                require_stage_field(stage, stage.from.as_deref(), "route requires from")?;
                if stage.on_success.is_none()
                    && stage.on_invalid.is_none()
                    && stage.on_skipped.is_none()
                    && stage.cases.is_empty()
                    && stage.default.is_none()
                {
                    return Err(stage_validation_error(
                        stage,
                        "route requires at least one target",
                    ));
                }
                Ok(())
            }
            "loop" => {
                require_stage_field(stage, stage.from.as_deref(), "loop requires from")?;
                if stage.max_iterations.unwrap_or(0) == 0 {
                    return Err(stage_validation_error(
                        stage,
                        "loop requires max_iterations greater than 0",
                    ));
                }
                if stage.break_on.is_none() {
                    return Err(stage_validation_error(stage, "loop requires break_on"));
                }
                if stage.body.is_empty() {
                    return Err(stage_validation_error(stage, "loop requires body"));
                }
                validate_loop_policy_fields(stage)?;
                for body_stage in &stage.body {
                    self.validate_stage(body_stage, plugin_stages, plugin_samplers)?;
                }
                Ok(())
            }
            "tool" => {
                require_stage_field(stage, stage.from.as_deref(), "tool requires from")?;
                Ok(())
            }
            "write" => {
                require_stage_field(stage, stage.from.as_deref(), "write requires from")?;
                Ok(())
            }
            other => Err(LlmffError::UnknownStage(other.to_string())),
        }
    }

    async fn execute_stage_with_timeout(
        &self,
        context: &ExecutionContext<'_>,
        stage: &StageSpec,
        statuses: &BTreeMap<String, StageStatus>,
        stream_writer: Option<&mut StageStreamWriter>,
        trace: Option<&mut Vec<TraceWriter>>,
    ) -> Result<StageOutcome, LlmffError> {
        let timeout_ms = stage.timeout_ms.or(context.options.default_timeout_ms);
        let run = self.execute_stage(context, stage, statuses, stream_writer, trace);
        if let Some(timeout_ms) = timeout_ms {
            return tokio::time::timeout(Duration::from_millis(timeout_ms), run)
                .await
                .map_err(|_| LlmffError::StageExecution {
                    stage_id: stage.id.clone(),
                    message: "stage timed out".to_string(),
                })?;
        }

        run.await
    }

    async fn execute_stage(
        &self,
        context: &ExecutionContext<'_>,
        stage: &StageSpec,
        statuses: &BTreeMap<String, StageStatus>,
        stream_writer: Option<&mut StageStreamWriter>,
        trace: Option<&mut Vec<TraceWriter>>,
    ) -> Result<StageOutcome, LlmffError> {
        if !should_execute_stage(stage, statuses)? {
            return Ok(StageOutcome::without_usage(StageStatus::Skipped));
        }

        match stage.op.as_str() {
            "load" => self
                .execute_load(context.manifest, stage, context.cwd)
                .map(StageOutcome::without_usage),
            "infer" => {
                self.execute_infer(
                    stage,
                    statuses,
                    context.plugins.samplers,
                    stream_writer,
                    context.options.default_retry,
                )
                .await
            }
            "validate_json" | "system" | "template" | "retrieve" | "rerank" => {
                let input = stage
                    .from
                    .as_ref()
                    .and_then(|parent| statuses.get(parent))
                    .and_then(success_value);
                execute_deterministic_stage(stage, input, context.cwd)
                    .map(StageOutcome::without_usage)
            }
            "cache" => self.execute_cache(stage, statuses, context.cwd),
            "repair" => {
                self.execute_repair(
                    stage,
                    statuses,
                    context.plugins.samplers,
                    context.options.default_retry,
                )
                .await
            }
            "route" => self
                .execute_route(stage, statuses)
                .map(StageOutcome::without_usage),
            "loop" => self
                .execute_loop(context, stage, statuses, stream_writer, trace)
                .await
                .map(StageOutcome::without_usage),
            "tool" => {
                self.execute_tool(
                    stage,
                    statuses,
                    context.cwd,
                    context.plugins.tool_transports,
                    context.options.default_retry,
                )
                .await
            }
            "write" => self
                .execute_write(stage, statuses, context.cwd)
                .map(StageOutcome::without_usage),
            other => {
                if let Some(plugin_stage_name) = plugin_stage_name(other) {
                    return self
                        .execute_plugin_stage(
                            stage,
                            statuses,
                            context.cwd,
                            context.plugins.stages,
                            plugin_stage_name,
                        )
                        .await
                        .map(StageOutcome::without_usage);
                }
                Err(LlmffError::UnknownStage(other.to_string()))
            }
        }
    }

    async fn execute_loop(
        &self,
        context: &ExecutionContext<'_>,
        stage: &StageSpec,
        statuses: &BTreeMap<String, StageStatus>,
        mut stream_writer: Option<&mut StageStreamWriter>,
        mut trace: Option<&mut Vec<TraceWriter>>,
    ) -> Result<StageStatus, LlmffError> {
        let source = stage
            .from
            .as_ref()
            .and_then(|parent| statuses.get(parent))
            .and_then(success_value)
            .ok_or_else(|| LlmffError::StageExecution {
                stage_id: stage.id.clone(),
                message: "loop requires successful input".to_string(),
            })?;
        let max_iterations = stage.max_iterations.unwrap_or(0);
        let ordered_body = crate::graph::order_loop_body_stages(stage)?;
        let final_stage_id = stage
            .final_output
            .as_ref()
            .map(|final_output| final_output.from.as_str())
            .or_else(|| stage.body.last().map(|body_stage| body_stage.id.as_str()))
            .ok_or_else(|| LlmffError::StageExecution {
                stage_id: stage.id.clone(),
                message: "loop requires final stage".to_string(),
            })?;
        let iteration_error_policy = stage.on_iteration_error.as_deref().unwrap_or("fail");
        let mut previous_body_statuses: BTreeMap<String, StageStatus> = BTreeMap::new();
        let mut last_body_statuses: BTreeMap<String, StageStatus> = BTreeMap::new();
        let mut latest_final_status = None;
        let mut latest_iteration_error = None;
        let mut stop_reason = LoopStopReason::MaxIterations;
        let mut iterations_run = 0usize;

        for iteration in 1..=max_iterations {
            iterations_run = iteration;
            let mut body_statuses = BTreeMap::new();
            body_statuses.insert("input".to_string(), StageStatus::Success(source.clone()));
            for (input_name, previous_stage_id) in &stage.carry {
                if let Some(previous_status) = previous_body_statuses.get(previous_stage_id) {
                    body_statuses.insert(input_name.clone(), previous_status.clone());
                }
            }

            for body_stage in &ordered_body {
                if iteration == 1 {
                    reject_missing_first_iteration_carry_alias(stage, body_stage, &body_statuses)?;
                }

                let trace_stage = loop_trace_stage(stage, body_stage);
                let loop_context = LoopTraceContext {
                    loop_id: &stage.id,
                    iteration,
                };
                let stage_started = if let Some(trace) = trace.as_deref_mut() {
                    Some(self.start_stage_trace_with_loop(
                        trace,
                        context.run_id,
                        &trace_stage,
                        &body_stage.id,
                        Some(loop_context),
                    )?)
                } else {
                    None
                };
                let outcome = match Box::pin(self.execute_stage_with_timeout(
                    context,
                    body_stage,
                    &body_statuses,
                    stream_writer.as_deref_mut(),
                    None,
                ))
                .await
                {
                    Ok(outcome) => outcome,
                    Err(error) => match iteration_error_policy {
                        "fail" => {
                            return Err(loop_trace_error(
                                error,
                                &trace_stage,
                                &body_stage.id,
                                loop_context,
                            ));
                        }
                        "break" => {
                            if let (Some(trace), Some(stage_started)) =
                                (trace.as_deref_mut(), stage_started)
                            {
                                self.finish_stage_trace_error_with_loop(
                                    trace,
                                    context.run_id,
                                    &trace_stage,
                                    &body_stage.id,
                                    stage_started,
                                    loop_context,
                                )?;
                            }
                            latest_iteration_error = Some((iteration, body_stage.id.clone()));
                            stop_reason = LoopStopReason::Error;
                            break;
                        }
                        "continue" => {
                            if let (Some(trace), Some(stage_started)) =
                                (trace.as_deref_mut(), stage_started)
                            {
                                self.finish_stage_trace_error_with_loop(
                                    trace,
                                    context.run_id,
                                    &trace_stage,
                                    &body_stage.id,
                                    stage_started,
                                    loop_context,
                                )?;
                            }
                            latest_iteration_error = Some((iteration, body_stage.id.clone()));
                            break;
                        }
                        _ => return Err(error),
                    },
                };
                if let (Some(trace), Some(stage_started)) = (trace.as_deref_mut(), stage_started) {
                    self.finish_stage_trace_with_loop(
                        trace,
                        context.run_id,
                        &trace_stage,
                        &body_stage.id,
                        &body_stage.id,
                        stage_started,
                        outcome,
                        &mut body_statuses,
                        Some(loop_context),
                    )?;
                } else {
                    body_statuses.insert(body_stage.id.clone(), outcome.status);
                }
                if body_stage.id == final_stage_id {
                    if let Some(final_status) = body_statuses
                        .get(final_stage_id)
                        .filter(|status| !matches!(status, StageStatus::Skipped))
                    {
                        latest_final_status = Some(final_status.clone());
                    }
                }
            }

            if matches!(stop_reason, LoopStopReason::Error) {
                if body_statuses.contains_key(final_stage_id) {
                    last_body_statuses = body_statuses;
                }
                break;
            }

            if latest_iteration_error
                .as_ref()
                .is_some_and(|(error_iteration, _)| *error_iteration == iteration)
            {
                previous_body_statuses = body_statuses;
                continue;
            }

            let should_break =
                evaluate_loop_break(stage.break_on.as_ref().unwrap(), &body_statuses)?;
            previous_body_statuses = body_statuses.clone();
            last_body_statuses = body_statuses;
            if should_break {
                stop_reason = LoopStopReason::BreakCondition;
                break;
            }
        }

        let final_status = last_body_statuses
            .get(final_stage_id)
            .filter(|status| !matches!(status, StageStatus::Skipped))
            .or(latest_final_status.as_ref())
            .ok_or_else(|| {
                if let Some((iteration, body_stage_id)) = latest_iteration_error.as_ref() {
                    let message = if matches!(stop_reason, LoopStopReason::Error) {
                        format!(
                            "loop `{}` stopped on iteration error at iteration {iteration} in body stage `{body_stage_id}` with no final output available",
                            stage.id
                        )
                    } else {
                        format!(
                            "loop `{}` produced no final output after iteration errors",
                            stage.id
                        )
                    };
                    return LlmffError::StageExecution {
                        stage_id: stage.id.clone(),
                        message,
                    };
                }

                LlmffError::StageExecution {
                    stage_id: stage.id.clone(),
                    message: format!("loop final stage `{final_stage_id}` did not run"),
                }
            })?;
        let final_value = loop_final_value(
            stage,
            final_stage_id,
            final_status,
            iterations_run,
            &stop_reason,
        )?;

        Ok(StageStatus::Success(final_value))
    }

    fn execute_load(
        &self,
        manifest: &Manifest,
        stage: &StageSpec,
        cwd: &Path,
    ) -> Result<StageStatus, LlmffError> {
        let input_name = stage
            .input
            .as_ref()
            .ok_or_else(|| LlmffError::StageExecution {
                stage_id: stage.id.clone(),
                message: "load requires input".to_string(),
            })?;
        let input = manifest
            .inputs
            .get(input_name)
            .ok_or_else(|| LlmffError::StageExecution {
                stage_id: stage.id.clone(),
                message: format!("unknown input `{input_name}`"),
            })?;
        let path = input
            .path
            .as_ref()
            .ok_or_else(|| LlmffError::StageExecution {
                stage_id: stage.id.clone(),
                message: "input requires path".to_string(),
            })?;
        let text = read_input(cwd, path)?;

        decode_input(stage, input_name, input.format.as_deref(), text)
    }

    async fn execute_infer(
        &self,
        stage: &StageSpec,
        statuses: &BTreeMap<String, StageStatus>,
        plugin_samplers: &BTreeMap<String, PluginSampler>,
        stream_writer: Option<&mut StageStreamWriter>,
        default_retry: RetryPolicy,
    ) -> Result<StageOutcome, LlmffError> {
        let messages = parent_messages(stage, statuses)?;
        let model = required_model(stage)?;
        let resolved = self.backend_for_model(model)?;
        let mut request = InferRequest {
            model: resolved.provider_model.to_string(),
            messages,
            temperature: stage.temperature,
            top_p: stage.top_p,
            max_tokens: stage.max_tokens,
            seed: stage.seed,
            response_format: stage.response_format.clone(),
            stop: stage.stop.clone(),
        };
        apply_plugin_sampler(stage, plugin_samplers, &mut request).await?;

        if let Some(writer) = stream_writer {
            if writer.stage_id == stage.id {
                let mut stream = resolved.backend.stream(request);
                let mut text = String::new();
                let mut usage = None;
                while let Some(chunk) = stream.next().await {
                    let chunk = chunk?;
                    if !chunk.delta.is_empty() {
                        writer.write_delta(&chunk.delta)?;
                        text.push_str(&chunk.delta);
                    }
                    if chunk.usage.is_some() {
                        usage = chunk.usage;
                    }
                    if chunk.done {
                        break;
                    }
                }

                return Ok(StageOutcome::with_streamed_usage(
                    StageStatus::Success(Value::Text(text)),
                    usage,
                ));
            }
        }

        let result =
            infer_with_retry(stage, resolved.backend.as_ref(), request, default_retry).await?;

        Ok(StageOutcome::with_usage_attempts(
            StageStatus::Success(Value::Text(result.response.text)),
            result.response.usage,
            result.attempts,
        ))
    }

    async fn execute_repair(
        &self,
        stage: &StageSpec,
        statuses: &BTreeMap<String, StageStatus>,
        plugin_samplers: &BTreeMap<String, PluginSampler>,
        default_retry: RetryPolicy,
    ) -> Result<StageOutcome, LlmffError> {
        let parent = stage
            .from
            .as_ref()
            .and_then(|parent| statuses.get(parent))
            .ok_or_else(|| LlmffError::StageExecution {
                stage_id: stage.id.clone(),
                message: "repair requires parent stage".to_string(),
            })?;

        match parent {
            StageStatus::Success(value) => Ok(StageOutcome::without_usage(StageStatus::Success(
                value.clone(),
            ))),
            StageStatus::Skipped => Ok(StageOutcome::without_usage(StageStatus::Skipped)),
            StageStatus::Invalid { value, errors } => {
                let model = required_model(stage)?;
                let resolved = self.backend_for_model(model)?;
                let mut request = InferRequest {
                    model: resolved.provider_model.to_string(),
                    messages: vec![Message {
                        role: "user".to_string(),
                        content: format!(
                            "Repair this output so it satisfies validation errors.\nErrors:\n{}\nOutput:\n{}",
                            errors.join("\n"),
                            serialize_value(value)?
                        ),
                    }],
                    temperature: stage.temperature,
                    top_p: stage.top_p,
                    max_tokens: stage.max_tokens,
                    seed: stage.seed,
                    response_format: stage.response_format.clone(),
                    stop: stage.stop.clone(),
                };
                apply_plugin_sampler(stage, plugin_samplers, &mut request).await?;
                let result =
                    infer_with_retry(stage, resolved.backend.as_ref(), request, default_retry)
                        .await?;

                Ok(StageOutcome::with_usage_attempts(
                    StageStatus::Success(Value::Text(result.response.text)),
                    result.response.usage,
                    result.attempts,
                ))
            }
        }
    }

    fn execute_route(
        &self,
        stage: &StageSpec,
        statuses: &BTreeMap<String, StageStatus>,
    ) -> Result<StageStatus, LlmffError> {
        let source_id = stage
            .from
            .as_ref()
            .ok_or_else(|| LlmffError::StageExecution {
                stage_id: stage.id.clone(),
                message: "route requires from".to_string(),
            })?;
        let source = statuses
            .get(source_id)
            .ok_or_else(|| LlmffError::StageExecution {
                stage_id: stage.id.clone(),
                message: format!("route source `{source_id}` is not available"),
            })?;

        let selected = if let Some(field) = &stage.field {
            select_field_route(stage, field, source)?
        } else {
            select_status_route(stage, source)
        };

        let target_id = selected.ok_or_else(|| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: "route did not match any target".to_string(),
        })?;
        statuses
            .get(target_id)
            .cloned()
            .ok_or_else(|| LlmffError::StageExecution {
                stage_id: stage.id.clone(),
                message: format!("route target `{target_id}` is not available"),
            })
    }

    async fn execute_tool(
        &self,
        stage: &StageSpec,
        statuses: &BTreeMap<String, StageStatus>,
        cwd: &Path,
        plugin_tool_transports: &BTreeMap<String, PluginToolTransport>,
        default_retry: RetryPolicy,
    ) -> Result<StageOutcome, LlmffError> {
        if let Some(command) = &stage.command {
            return execute_command_tool(stage, statuses, cwd, command)
                .await
                .map(StageOutcome::without_usage);
        }
        if stage.url.is_some() {
            let result = execute_http_tool_with_retry(stage, statuses, default_retry).await?;
            return Ok(StageOutcome::with_usage_attempts(
                result.status,
                None,
                result.attempts,
            ));
        }
        if let Some(transport) = &stage.transport {
            let plugin_transport = plugin_tool_transports.get(transport).ok_or_else(|| {
                LlmffError::StageExecution {
                    stage_id: stage.id.clone(),
                    message: format!("unknown plugin tool transport `{transport}`"),
                }
            })?;
            return execute_command_tool(
                stage,
                statuses,
                cwd,
                &[plugin_transport.entrypoint.to_string_lossy().into_owned()],
            )
            .await
            .map(StageOutcome::without_usage);
        }

        Err(LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: "tool requires command, url, or plugin transport".to_string(),
        })
    }

    async fn execute_plugin_stage(
        &self,
        stage: &StageSpec,
        statuses: &BTreeMap<String, StageStatus>,
        cwd: &Path,
        plugin_stages: &BTreeMap<String, PluginStage>,
        plugin_stage_name: &str,
    ) -> Result<StageStatus, LlmffError> {
        let plugin_stage =
            plugin_stages
                .get(plugin_stage_name)
                .ok_or_else(|| LlmffError::StageExecution {
                    stage_id: stage.id.clone(),
                    message: format!("unknown plugin stage `{plugin_stage_name}`"),
                })?;
        execute_command_stage(
            stage,
            statuses,
            cwd,
            &[plugin_stage.entrypoint.to_string_lossy().into_owned()],
            "plugin stage",
        )
        .await
    }

    fn execute_write(
        &self,
        stage: &StageSpec,
        statuses: &BTreeMap<String, StageStatus>,
        cwd: &Path,
    ) -> Result<StageStatus, LlmffError> {
        let parent = stage
            .from
            .as_ref()
            .ok_or_else(|| LlmffError::StageExecution {
                stage_id: stage.id.clone(),
                message: "write requires parent stage".to_string(),
            })?;
        let status = statuses
            .get(parent)
            .ok_or_else(|| LlmffError::StageExecution {
                stage_id: stage.id.clone(),
                message: format!("unknown parent stage `{parent}`"),
            })?;
        let value = match status {
            StageStatus::Success(value) => value,
            StageStatus::Invalid { errors, .. } => {
                return Err(LlmffError::StageExecution {
                    stage_id: stage.id.clone(),
                    message: format!("parent stage is invalid: {}", errors.join("; ")),
                });
            }
            StageStatus::Skipped => {
                return Err(LlmffError::StageExecution {
                    stage_id: stage.id.clone(),
                    message: "parent stage was skipped".to_string(),
                });
            }
        };
        let path = stage
            .path
            .as_deref()
            .ok_or_else(|| LlmffError::StageExecution {
                stage_id: stage.id.clone(),
                message: "write requires path".to_string(),
            })?;

        write_output(cwd, path, &serialize_value(value)?)?;

        Ok(StageStatus::Success(value.clone()))
    }

    fn execute_cache(
        &self,
        stage: &StageSpec,
        statuses: &BTreeMap<String, StageStatus>,
        cwd: &Path,
    ) -> Result<StageOutcome, LlmffError> {
        let value = parent_success_value(stage, statuses)?;
        let cache_path = stage
            .path
            .clone()
            .unwrap_or_else(|| ".llmff/cache".to_string());
        let cache_dir = resolve_path(cwd, &cache_path);
        let cache_file = cache_dir.join(format!("{}.json", cache_digest(stage, value)?));
        let cache_policy = stage.cache_policy.as_deref().unwrap_or("read");

        if cache_policy == "bypass" {
            return Ok(StageOutcome::with_cache(
                StageStatus::Success(value.clone()),
                false,
                cache_path,
            ));
        }

        if cache_policy != "refresh" && cache_file.exists() {
            let source = std::fs::read_to_string(&cache_file).map_err(|error| {
                LlmffError::StageExecution {
                    stage_id: stage.id.clone(),
                    message: format!(
                        "failed to read cache file `{}`: {error}",
                        cache_file.display()
                    ),
                }
            })?;
            let record: CacheRecord =
                serde_json::from_str(&source).map_err(|error| LlmffError::StageExecution {
                    stage_id: stage.id.clone(),
                    message: format!("invalid cache file `{}`: {error}", cache_file.display()),
                })?;
            if record.version != CACHE_RECORD_VERSION {
                return Err(LlmffError::StageExecution {
                    stage_id: stage.id.clone(),
                    message: format!(
                        "unsupported cache file `{}` version {}",
                        cache_file.display(),
                        record.version
                    ),
                });
            }

            return Ok(StageOutcome::with_cache(
                StageStatus::Success(record.value),
                true,
                cache_path,
            ));
        }

        std::fs::create_dir_all(&cache_dir).map_err(|error| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!(
                "failed to create cache directory `{}`: {error}",
                cache_dir.display()
            ),
        })?;
        let record = CacheRecord {
            version: CACHE_RECORD_VERSION,
            value: value.clone(),
        };
        let encoded = serde_json::to_vec_pretty(&record).map_err(LlmffError::Json)?;
        write_cache_file(stage, &cache_file, &encoded)?;

        Ok(StageOutcome::with_cache(
            StageStatus::Success(value.clone()),
            false,
            cache_path,
        ))
    }

    fn backend_for_model<'a>(&'a self, model: &'a str) -> Result<ResolvedBackend<'a>, LlmffError> {
        if let Some(backend) = self.backends.get(model) {
            return Ok(ResolvedBackend {
                backend,
                provider_model: model,
            });
        }

        if let Some((alias, provider_model)) = model.split_once(':') {
            if let Some(backend) = self.backends.get(alias) {
                return Ok(ResolvedBackend {
                    backend,
                    provider_model,
                });
            }
        }

        Err(LlmffError::Backend(format!(
            "no backend configured for `{model}`"
        )))
    }

    fn trace_metadata(
        &self,
        stage: &StageSpec,
        status: &StageStatus,
        usage: Option<&UsageMetadata>,
    ) -> TraceMetadata {
        let mut metadata = TraceMetadata::default();

        if matches!(stage.op.as_str(), "infer" | "repair") {
            if let Some(model) = &stage.model {
                metadata.model = Some(model.clone());
                if let Some(resolved) = self.resolve_backend_metadata(model) {
                    metadata.backend = Some(resolved.backend_alias);
                    metadata.provider_model = Some(resolved.provider_model);
                }
            }
        }

        if let StageStatus::Invalid { errors, .. } = status {
            metadata.validation_errors = Some(errors.clone());
        }

        if stage.op == "tool" {
            if let Some(command) = &stage.command {
                if let Some(program) = command.first() {
                    metadata.tool_kind = Some("command".to_string());
                    metadata.tool_target = Some(program.clone());
                }
            } else if let Some(url) = &stage.url {
                metadata.tool_kind = Some("http".to_string());
                metadata.tool_target = Some(url.clone());
            } else if let Some(transport) = &stage.transport {
                metadata.tool_kind = Some("plugin".to_string());
                metadata.tool_target = Some(transport.clone());
            }
        }

        if stage.op == "write" {
            metadata.output_path = stage.path.clone();
        }

        if let Some(usage) = usage {
            metadata.prompt_tokens = usage.prompt_tokens;
            metadata.completion_tokens = usage.completion_tokens;
            metadata.total_tokens = usage.total_tokens;
        }

        metadata
    }

    fn resolve_backend_metadata(&self, model: &str) -> Option<ResolvedBackendMetadata> {
        if self.backends.contains_key(model) {
            return Some(ResolvedBackendMetadata {
                backend_alias: model.to_string(),
                provider_model: model.to_string(),
            });
        }

        let (alias, provider_model) = model.split_once(':')?;
        self.backends
            .contains_key(alias)
            .then(|| ResolvedBackendMetadata {
                backend_alias: alias.to_string(),
                provider_model: provider_model.to_string(),
            })
    }
}

fn loop_trace_stage(loop_stage: &StageSpec, body_stage: &StageSpec) -> StageSpec {
    let mut trace_stage = body_stage.clone();
    trace_stage.id = format!("{}.{}", loop_stage.id, body_stage.id);
    trace_stage
}

fn loop_trace_error(
    error: LlmffError,
    trace_stage: &StageSpec,
    loop_stage_id: &str,
    loop_context: LoopTraceContext<'_>,
) -> LlmffError {
    LlmffError::LoopStageExecution {
        stage_id: trace_stage.id.clone(),
        loop_id: loop_context.loop_id.to_string(),
        loop_iteration: loop_context.iteration,
        loop_stage_id: loop_stage_id.to_string(),
        source: Box::new(error),
    }
}

fn require_stage_field<'a>(
    stage: &StageSpec,
    value: Option<&'a str>,
    message: &'static str,
) -> Result<&'a str, LlmffError> {
    value.ok_or_else(|| stage_validation_error(stage, message))
}

fn stage_validation_error(stage: &StageSpec, message: impl Into<String>) -> LlmffError {
    LlmffError::StageExecution {
        stage_id: stage.id.clone(),
        message: message.into(),
    }
}

fn retry_policy(stage: &StageSpec, default_retry: RetryPolicy) -> RetryPolicy {
    stage
        .retry
        .as_ref()
        .map(retry_policy_from_spec)
        .unwrap_or(default_retry)
}

fn retry_policy_from_spec(spec: &RetrySpec) -> RetryPolicy {
    RetryPolicy {
        attempts: spec.attempts.max(1),
        backoff_ms: spec.backoff_ms.unwrap_or(0),
    }
}

fn validate_input_formats(manifest: &Manifest) -> Result<(), LlmffError> {
    for (id, input) in &manifest.inputs {
        if input_format(input.format.as_deref()).is_none() {
            let format = input.format.as_deref().unwrap_or_default();
            return Err(LlmffError::GraphValidation(format!(
                "input `{id}` has unsupported format `{format}`"
            )));
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputFormat {
    Text,
    Json,
}

fn input_format(format: Option<&str>) -> Option<InputFormat> {
    match format.unwrap_or("text") {
        "text" => Some(InputFormat::Text),
        "json" => Some(InputFormat::Json),
        _ => None,
    }
}

fn decode_input(
    stage: &StageSpec,
    input_name: &str,
    format: Option<&str>,
    source: String,
) -> Result<StageStatus, LlmffError> {
    match input_format(format).ok_or_else(|| LlmffError::StageExecution {
        stage_id: stage.id.clone(),
        message: format!(
            "input `{input_name}` has unsupported format `{}`",
            format.unwrap_or_default()
        ),
    })? {
        InputFormat::Text => Ok(StageStatus::Success(Value::Text(source))),
        InputFormat::Json => serde_json::from_str(&source)
            .map(Value::Json)
            .map(StageStatus::Success)
            .map_err(|error| LlmffError::StageExecution {
                stage_id: stage.id.clone(),
                message: format!("input `{input_name}` is not valid JSON: {error}"),
            }),
    }
}

fn validate_sampling_parameters(stage: &StageSpec) -> Result<(), LlmffError> {
    validate_execution_options(stage)?;
    if let Some(temperature) = stage.temperature {
        if temperature < 0.0 {
            return Err(stage_validation_error(
                stage,
                "temperature must be greater than or equal to 0",
            ));
        }
    }
    if let Some(top_p) = stage.top_p {
        if !(0.0..=1.0).contains(&top_p) {
            return Err(stage_validation_error(
                stage,
                "top_p must be between 0 and 1",
            ));
        }
    }
    if let Some(0) = stage.max_tokens {
        return Err(stage_validation_error(
            stage,
            "max_tokens must be greater than 0",
        ));
    }
    if let Some(response_format) = stage.response_format.as_deref() {
        if response_format != "json" {
            return Err(stage_validation_error(
                stage,
                "response_format must be json",
            ));
        }
    }
    if stage.stop.iter().any(|stop| stop.is_empty()) {
        return Err(stage_validation_error(
            stage,
            "stop sequences cannot be empty",
        ));
    }

    Ok(())
}

fn validate_execution_options(stage: &StageSpec) -> Result<(), LlmffError> {
    if stage.timeout_ms == Some(0) {
        return Err(stage_validation_error(
            stage,
            "timeout_ms must be greater than 0",
        ));
    }
    if let Some(retry) = &stage.retry {
        if retry.attempts == 0 {
            return Err(stage_validation_error(
                stage,
                "retry attempts must be greater than 0",
            ));
        }
    }
    if let Some(policy) = stage.cache_policy.as_deref() {
        if !matches!(policy, "read" | "refresh" | "bypass") {
            return Err(stage_validation_error(
                stage,
                "cache_policy must be read, refresh, or bypass",
            ));
        }
    }

    Ok(())
}

fn validate_loop_policy_fields(stage: &StageSpec) -> Result<(), LlmffError> {
    if let Some(policy) = stage.on_iteration_error.as_deref() {
        if !matches!(policy, "fail" | "continue" | "break") {
            return Err(stage_validation_error(
                stage,
                "on_iteration_error must be fail, continue, or break",
            ));
        }
    }
    if let Some(retain) = stage.retain_iterations.as_deref() {
        if !matches!(retain, "none" | "summaries" | "all") {
            return Err(stage_validation_error(
                stage,
                "retain_iterations must be none, summaries, or all",
            ));
        }
    }
    if let Some(final_output) = &stage.final_output {
        if let Some(require_status) = final_output.require_status.as_deref() {
            if !matches!(require_status, "success" | "invalid" | "any") {
                return Err(stage_validation_error(
                    stage,
                    "final.require_status must be success, invalid, or any",
                ));
            }
        }
    }
    Ok(())
}

fn validate_sampler_reference(
    stage: &StageSpec,
    plugin_samplers: &BTreeMap<String, PluginSampler>,
) -> Result<(), LlmffError> {
    let Some(sampler) = stage.sampler.as_deref() else {
        return Ok(());
    };
    if plugin_samplers.contains_key(sampler) {
        Ok(())
    } else {
        Err(stage_validation_error(
            stage,
            format!("unknown plugin sampler `{sampler}`"),
        ))
    }
}

fn reject_sampler_on_non_model_stage(stage: &StageSpec) -> Result<(), LlmffError> {
    if stage.sampler.is_some() {
        return Err(stage_validation_error(
            stage,
            "sampler is only supported on infer and repair stages",
        ));
    }
    Ok(())
}

fn validate_retrieve_strategy(stage: &StageSpec) -> Result<(), LlmffError> {
    validate_retrieval_strategy(stage, "retrieve")
}

fn validate_rerank_strategy(stage: &StageSpec) -> Result<(), LlmffError> {
    validate_retrieval_strategy(stage, "rerank")
}

fn validate_retrieval_strategy(stage: &StageSpec, operation: &str) -> Result<(), LlmffError> {
    match stage.strategy.as_deref().unwrap_or("lexical") {
        "lexical" | "embedding" | "command" => Ok(()),
        strategy => Err(stage_validation_error(
            stage,
            format!(
                "{operation} strategy must be lexical, embedding, or command, got `{strategy}`"
            ),
        )),
    }
}

fn is_command_retrieval_strategy(stage: &StageSpec) -> bool {
    stage.strategy.as_deref() == Some("command")
}

fn validate_when_condition(stage: &StageSpec) -> Result<(), LlmffError> {
    let Some(condition) = stage.when.as_deref() else {
        return Ok(());
    };

    match condition {
        "success" | "invalid" | "skipped" => {
            if stage.from.is_none() {
                return Err(stage_validation_error(stage, "when requires from"));
            }
            Ok(())
        }
        other => Err(stage_validation_error(
            stage,
            format!("unknown when condition `{other}`"),
        )),
    }
}

fn should_execute_stage(
    stage: &StageSpec,
    statuses: &BTreeMap<String, StageStatus>,
) -> Result<bool, LlmffError> {
    let Some(condition) = stage.when.as_deref() else {
        return Ok(true);
    };
    let parent_id = stage
        .from
        .as_ref()
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: "when requires parent stage".to_string(),
        })?;
    let parent = statuses
        .get(parent_id)
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!("unknown parent stage `{parent_id}`"),
        })?;

    match condition {
        "success" => Ok(matches!(parent, StageStatus::Success(_))),
        "invalid" => Ok(matches!(parent, StageStatus::Invalid { .. })),
        "skipped" => Ok(matches!(parent, StageStatus::Skipped)),
        other => Err(stage_validation_error(
            stage,
            format!("unknown when condition `{other}`"),
        )),
    }
}

fn plugin_stage_name(op: &str) -> Option<&str> {
    op.strip_prefix("plugin:").filter(|name| !name.is_empty())
}

const CACHE_RECORD_VERSION: u32 = 1;

#[derive(Debug, Deserialize, Serialize)]
struct CacheRecord {
    version: u32,
    value: Value,
}

fn parent_success_value<'a>(
    stage: &StageSpec,
    statuses: &'a BTreeMap<String, StageStatus>,
) -> Result<&'a Value, LlmffError> {
    let parent = stage
        .from
        .as_ref()
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: "cache requires parent stage".to_string(),
        })?;
    let status = statuses
        .get(parent)
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!("unknown parent stage `{parent}`"),
        })?;

    match status {
        StageStatus::Success(value) => Ok(value),
        StageStatus::Invalid { errors, .. } => Err(LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!("parent stage is invalid: {}", errors.join("; ")),
        }),
        StageStatus::Skipped => Err(LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: "parent stage was skipped".to_string(),
        }),
    }
}

fn cache_digest(stage: &StageSpec, value: &Value) -> Result<String, LlmffError> {
    let preimage = if let Some(key) = &stage.key {
        serde_json::json!({
            "version": CACHE_RECORD_VERSION,
            "stage_id": stage.id,
            "key": key,
        })
    } else {
        serde_json::json!({
            "version": CACHE_RECORD_VERSION,
            "stage_id": stage.id,
            "key": stage.id,
            "value": value,
        })
    };
    let encoded = serde_json::to_vec(&preimage).map_err(LlmffError::Json)?;
    let digest = Sha256::digest(encoded);

    Ok(digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>())
}

fn write_cache_file(
    stage: &StageSpec,
    cache_file: &Path,
    encoded: &[u8],
) -> Result<(), LlmffError> {
    let tmp_file = cache_file.with_extension(format!(
        "json.tmp.{}.{}",
        std::process::id(),
        timestamp_ms()
    ));
    std::fs::write(&tmp_file, encoded).map_err(|error| LlmffError::StageExecution {
        stage_id: stage.id.clone(),
        message: format!(
            "failed to write cache file `{}`: {error}",
            tmp_file.display()
        ),
    })?;
    std::fs::rename(&tmp_file, cache_file).map_err(|error| LlmffError::StageExecution {
        stage_id: stage.id.clone(),
        message: format!(
            "failed to move cache file `{}` into `{}`: {error}",
            tmp_file.display(),
            cache_file.display()
        ),
    })?;

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StageValueKind {
    Any,
    Text,
    Json,
}

impl StageValueKind {
    fn label(self) -> &'static str {
        match self {
            Self::Any => "any",
            Self::Text => "text",
            Self::Json => "json",
        }
    }
}

fn validate_stage_types(graph: &Graph, manifest: &Manifest) -> Result<(), LlmffError> {
    let mut kinds = BTreeMap::new();

    for stage in graph.stages() {
        if let Some(field) = &stage.field {
            if stage.op == "route" {
                validate_field_route_source_kind(stage, field, &kinds)?;
            }
        }

        let kind = infer_stage_value_kind(stage, manifest, &kinds);
        kinds.insert(stage.id.clone(), kind);
    }

    Ok(())
}

fn validate_field_route_source_kind(
    stage: &StageSpec,
    _field: &str,
    kinds: &BTreeMap<String, StageValueKind>,
) -> Result<(), LlmffError> {
    let Some(source_id) = stage.from.as_ref() else {
        return Ok(());
    };
    let kind = kinds.get(source_id).copied().unwrap_or(StageValueKind::Any);
    if kind == StageValueKind::Text {
        return Err(stage_validation_error(
            stage,
            format!(
                "field route requires JSON source `{source_id}`, got {}",
                kind.label()
            ),
        ));
    }

    Ok(())
}

fn infer_stage_value_kind(
    stage: &StageSpec,
    manifest: &Manifest,
    kinds: &BTreeMap<String, StageValueKind>,
) -> StageValueKind {
    if plugin_stage_name(&stage.op).is_some() {
        return StageValueKind::Text;
    }

    match stage.op.as_str() {
        "load" => stage
            .input
            .as_ref()
            .and_then(|input_id| manifest.inputs.get(input_id))
            .and_then(|input| input_format(input.format.as_deref()))
            .map(|format| match format {
                InputFormat::Text => StageValueKind::Text,
                InputFormat::Json => StageValueKind::Json,
            })
            .unwrap_or(StageValueKind::Text),
        "validate_json" | "retrieve" | "rerank" => StageValueKind::Json,
        "write" | "cache" => stage
            .from
            .as_ref()
            .and_then(|parent| kinds.get(parent))
            .copied()
            .unwrap_or(StageValueKind::Any),
        "route" => StageValueKind::Any,
        "system" | "template" | "infer" | "repair" | "tool" => StageValueKind::Text,
        _ => StageValueKind::Any,
    }
}

#[derive(Default)]
struct TraceMetadata {
    model: Option<String>,
    backend: Option<String>,
    provider_model: Option<String>,
    validation_errors: Option<Vec<String>>,
    tool_kind: Option<String>,
    tool_target: Option<String>,
    output_path: Option<String>,
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
    total_tokens: Option<u64>,
}

struct ResolvedBackendMetadata {
    backend_alias: String,
    provider_model: String,
}

#[derive(Debug, Deserialize)]
struct SamplerOverrides {
    temperature: Option<f32>,
    top_p: Option<f32>,
    max_tokens: Option<u32>,
    seed: Option<u64>,
    response_format: Option<String>,
    stop: Option<Vec<String>>,
}

async fn apply_plugin_sampler(
    stage: &StageSpec,
    plugin_samplers: &BTreeMap<String, PluginSampler>,
    request: &mut InferRequest,
) -> Result<(), LlmffError> {
    let Some(sampler_name) = stage.sampler.as_deref() else {
        return Ok(());
    };
    let sampler = plugin_samplers
        .get(sampler_name)
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!("unknown plugin sampler `{sampler_name}`"),
        })?;
    let encoded = serde_json::to_vec(request).map_err(LlmffError::Json)?;
    let mut child = Command::new(&sampler.entrypoint)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!("failed to start plugin sampler `{sampler_name}`: {error}"),
        })?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!("failed to open plugin sampler `{sampler_name}` stdin"),
        })?;
    stdin
        .write_all(&encoded)
        .await
        .map_err(|error| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!("failed to write plugin sampler `{sampler_name}` request: {error}"),
        })?;
    drop(stdin);

    let output = child
        .wait_with_output()
        .await
        .map_err(|error| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!("failed to wait for plugin sampler `{sampler_name}`: {error}"),
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!(
                "plugin sampler `{sampler_name}` exited with status {}: {}",
                output.status,
                stderr.trim()
            ),
        });
    }

    let overrides: SamplerOverrides =
        serde_json::from_slice(&output.stdout).map_err(|error| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!("plugin sampler `{sampler_name}` returned invalid JSON: {error}"),
        })?;
    validate_sampler_overrides(stage, sampler_name, &overrides)?;
    apply_sampler_overrides(request, overrides);
    Ok(())
}

fn validate_sampler_overrides(
    stage: &StageSpec,
    sampler_name: &str,
    overrides: &SamplerOverrides,
) -> Result<(), LlmffError> {
    if overrides
        .temperature
        .is_some_and(|temperature| temperature < 0.0)
    {
        return Err(plugin_sampler_override_error(
            stage,
            sampler_name,
            "temperature must be greater than or equal to 0",
        ));
    }
    if overrides
        .top_p
        .is_some_and(|top_p| !(0.0..=1.0).contains(&top_p))
    {
        return Err(plugin_sampler_override_error(
            stage,
            sampler_name,
            "top_p must be between 0 and 1",
        ));
    }
    if overrides.max_tokens == Some(0) {
        return Err(plugin_sampler_override_error(
            stage,
            sampler_name,
            "max_tokens must be greater than 0",
        ));
    }
    if overrides
        .response_format
        .as_deref()
        .is_some_and(|response_format| response_format != "json")
    {
        return Err(plugin_sampler_override_error(
            stage,
            sampler_name,
            "response_format must be json",
        ));
    }
    if overrides
        .stop
        .as_ref()
        .is_some_and(|stop| stop.iter().any(|sequence| sequence.is_empty()))
    {
        return Err(plugin_sampler_override_error(
            stage,
            sampler_name,
            "stop sequences cannot be empty",
        ));
    }
    Ok(())
}

fn plugin_sampler_override_error(
    stage: &StageSpec,
    sampler_name: &str,
    message: &str,
) -> LlmffError {
    LlmffError::StageExecution {
        stage_id: stage.id.clone(),
        message: format!("plugin sampler `{sampler_name}` returned invalid overrides: {message}"),
    }
}

fn apply_sampler_overrides(request: &mut InferRequest, overrides: SamplerOverrides) {
    if let Some(temperature) = overrides.temperature {
        request.temperature = Some(temperature);
    }
    if let Some(top_p) = overrides.top_p {
        request.top_p = Some(top_p);
    }
    if let Some(max_tokens) = overrides.max_tokens {
        request.max_tokens = Some(max_tokens);
    }
    if let Some(seed) = overrides.seed {
        request.seed = Some(seed);
    }
    if let Some(response_format) = overrides.response_format {
        request.response_format = Some(response_format);
    }
    if let Some(stop) = overrides.stop {
        request.stop = stop;
    }
}

fn load_plugin_tool_transports(
    plugin_dirs: &[PathBuf],
) -> Result<BTreeMap<String, PluginToolTransport>, LlmffError> {
    let mut transports = BTreeMap::new();
    for plugin_dir in plugin_dirs {
        for transport in discover_plugin_tool_transports(plugin_dir)? {
            if transports.contains_key(&transport.name) {
                return Err(LlmffError::Config(format!(
                    "duplicate plugin tool transport `{}`",
                    transport.name
                )));
            }
            transports.insert(transport.name.clone(), transport);
        }
    }
    Ok(transports)
}

fn load_plugin_stages(
    plugin_dirs: &[PathBuf],
) -> Result<BTreeMap<String, PluginStage>, LlmffError> {
    let mut stages = BTreeMap::new();
    for plugin_dir in plugin_dirs {
        for stage in discover_plugin_stages(plugin_dir)? {
            if stages.contains_key(&stage.name) {
                return Err(LlmffError::Config(format!(
                    "duplicate plugin stage `{}`",
                    stage.name
                )));
            }
            stages.insert(stage.name.clone(), stage);
        }
    }
    Ok(stages)
}

fn load_plugin_samplers(
    plugin_dirs: &[PathBuf],
) -> Result<BTreeMap<String, PluginSampler>, LlmffError> {
    let mut samplers = BTreeMap::new();
    for plugin_dir in plugin_dirs {
        for sampler in discover_plugin_samplers(plugin_dir)? {
            if samplers.contains_key(&sampler.name) {
                return Err(LlmffError::Config(format!(
                    "duplicate plugin sampler `{}`",
                    sampler.name
                )));
            }
            samplers.insert(sampler.name.clone(), sampler);
        }
    }
    Ok(samplers)
}

async fn execute_command_tool(
    stage: &StageSpec,
    statuses: &BTreeMap<String, StageStatus>,
    cwd: &Path,
    command: &[String],
) -> Result<StageStatus, LlmffError> {
    execute_command_stage(stage, statuses, cwd, command, "tool command").await
}

async fn execute_command_stage(
    stage: &StageSpec,
    statuses: &BTreeMap<String, StageStatus>,
    cwd: &Path,
    command: &[String],
    label: &str,
) -> Result<StageStatus, LlmffError> {
    let input = parent_text(stage, statuses)?;
    let program = command.first().ok_or_else(|| LlmffError::StageExecution {
        stage_id: stage.id.clone(),
        message: format!("{label} cannot be empty"),
    })?;
    let mut child = Command::new(resolve_command_path(cwd, program))
        .args(&command[1..])
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!("failed to start {label} `{program}`: {error}"),
        })?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!("failed to open {label} stdin"),
        })?;
    stdin
        .write_all(input.as_bytes())
        .await
        .map_err(|error| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!("failed to write {label} stdin: {error}"),
        })?;
    drop(stdin);

    let output = child
        .wait_with_output()
        .await
        .map_err(|error| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!("failed to wait for {label} `{program}`: {error}"),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!(
                "{label} exited with status {}: {}",
                output.status,
                stderr.trim()
            ),
        });
    }

    let stdout = String::from_utf8(output.stdout).map_err(|error| LlmffError::StageExecution {
        stage_id: stage.id.clone(),
        message: format!("{label} stdout was not valid UTF-8: {error}"),
    })?;
    Ok(StageStatus::Success(Value::Text(stdout)))
}

fn resolve_command_path(cwd: &Path, program: &str) -> PathBuf {
    let path = Path::new(program);
    if path.is_relative() && path.components().count() > 1 {
        cwd.join(path)
    } else {
        path.to_path_buf()
    }
}

async fn execute_http_tool(
    stage: &StageSpec,
    statuses: &BTreeMap<String, StageStatus>,
) -> Result<StageStatus, LlmffError> {
    let input = parent_text(stage, statuses)?;
    let method = stage
        .method
        .as_deref()
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: "http tool requires method".to_string(),
        })?
        .parse::<reqwest::Method>()
        .map_err(|error| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!("invalid http tool method: {error}"),
        })?;
    let url = stage
        .url
        .as_deref()
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: "http tool requires url".to_string(),
        })?;

    let client = reqwest::Client::new();
    let mut request = client.request(method.clone(), url);
    for (name, value) in &stage.headers {
        request = request.header(name, value);
    }
    if method_allows_body(&method) {
        request = request.body(input);
    }

    let response = request
        .send()
        .await
        .map_err(|error| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!("http tool request failed: {error}"),
        })?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!("failed to read http tool response: {error}"),
        })?;

    if !status.is_success() {
        return Err(LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!("http tool returned status {status}: {body}"),
        });
    }

    Ok(StageStatus::Success(Value::Text(body)))
}

async fn infer_with_retry(
    stage: &StageSpec,
    backend: &dyn Backend,
    request: InferRequest,
    default_retry: RetryPolicy,
) -> Result<InferAttemptResult, LlmffError> {
    let policy = retry_policy(stage, default_retry);
    let mut attempt = 1usize;

    loop {
        match backend.infer(request.clone()).await {
            Ok(response) => {
                return Ok(InferAttemptResult {
                    response,
                    attempts: attempt,
                })
            }
            Err(error) if attempt < policy.attempts => {
                attempt += 1;
                sleep_for_retry(policy.backoff_ms).await;
                drop(error);
            }
            Err(error) => return Err(error),
        }
    }
}

async fn execute_http_tool_with_retry(
    stage: &StageSpec,
    statuses: &BTreeMap<String, StageStatus>,
    default_retry: RetryPolicy,
) -> Result<ToolAttemptResult, LlmffError> {
    let policy = retry_policy(stage, default_retry);
    let mut attempt = 1usize;

    loop {
        match execute_http_tool(stage, statuses).await {
            Ok(status) => {
                return Ok(ToolAttemptResult {
                    status,
                    attempts: attempt,
                })
            }
            Err(error) if attempt < policy.attempts && is_retryable_http_tool_error(&error) => {
                attempt += 1;
                sleep_for_retry(policy.backoff_ms).await;
                drop(error);
            }
            Err(error) => return Err(error),
        }
    }
}

fn is_retryable_http_tool_error(error: &LlmffError) -> bool {
    let LlmffError::StageExecution { message, .. } = error else {
        return false;
    };
    message.starts_with("http tool request failed:")
        || message.contains("http tool returned status 500")
        || message.contains("http tool returned status 502")
        || message.contains("http tool returned status 503")
        || message.contains("http tool returned status 504")
}

async fn sleep_for_retry(backoff_ms: u64) {
    if backoff_ms > 0 {
        tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
    }
}

fn method_allows_body(method: &reqwest::Method) -> bool {
    matches!(
        *method,
        reqwest::Method::POST | reqwest::Method::PUT | reqwest::Method::PATCH
    )
}

fn select_status_route<'a>(stage: &'a StageSpec, source: &StageStatus) -> Option<&'a str> {
    match source {
        StageStatus::Success(_) => stage.on_success.as_deref().or(stage.default.as_deref()),
        StageStatus::Invalid { .. } => stage.on_invalid.as_deref().or(stage.default.as_deref()),
        StageStatus::Skipped => stage.on_skipped.as_deref().or(stage.default.as_deref()),
    }
}

fn select_field_route<'a>(
    stage: &'a StageSpec,
    field: &str,
    source: &StageStatus,
) -> Result<Option<&'a str>, LlmffError> {
    let StageStatus::Success(Value::Json(serde_json::Value::Object(object))) = source else {
        return Err(LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: "field route requires successful JSON object source".to_string(),
        });
    };
    let value = object
        .get(field)
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!("field route source is missing field `{field}`"),
        })?;
    let key = route_value_key(value).ok_or_else(|| LlmffError::StageExecution {
        stage_id: stage.id.clone(),
        message: format!("field route `{field}` must be string, number, or boolean"),
    })?;

    Ok(stage
        .cases
        .get(&key)
        .map(String::as_str)
        .or(stage.default.as_deref()))
}

fn route_value_key(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Number(number) => Some(number.to_string()),
        serde_json::Value::Bool(boolean) => Some(boolean.to_string()),
        serde_json::Value::Null | serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            None
        }
    }
}

struct ResolvedBackend<'a> {
    backend: &'a Arc<dyn Backend>,
    provider_model: &'a str,
}

impl std::ops::Deref for ResolvedBackend<'_> {
    type Target = Arc<dyn Backend>;

    fn deref(&self) -> &Self::Target {
        self.backend
    }
}

fn success_value(status: &StageStatus) -> Option<Value> {
    match status {
        StageStatus::Success(value) => Some(value.clone()),
        StageStatus::Invalid { .. } | StageStatus::Skipped => None,
    }
}

fn parent_text(
    stage: &StageSpec,
    statuses: &BTreeMap<String, StageStatus>,
) -> Result<String, LlmffError> {
    let parent = stage
        .from
        .as_ref()
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: "stage requires parent input".to_string(),
        })?;
    let status = statuses
        .get(parent)
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!("unknown parent stage `{parent}`"),
        })?;
    match status {
        StageStatus::Success(Value::Text(text)) => Ok(text.clone()),
        StageStatus::Success(Value::Messages(messages)) => Ok(render_messages_as_text(messages)),
        StageStatus::Success(Value::Json(json)) => Ok(json.to_string()),
        StageStatus::Invalid { errors, .. } => Err(LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!("parent stage is invalid: {}", errors.join("; ")),
        }),
        StageStatus::Skipped => Err(LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: "parent stage was skipped".to_string(),
        }),
    }
}

fn parent_messages(
    stage: &StageSpec,
    statuses: &BTreeMap<String, StageStatus>,
) -> Result<Vec<Message>, LlmffError> {
    let parent = stage
        .from
        .as_ref()
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: "stage requires parent input".to_string(),
        })?;
    let status = statuses
        .get(parent)
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!("unknown parent stage `{parent}`"),
        })?;
    match status {
        StageStatus::Success(Value::Messages(messages)) => Ok(messages.clone()),
        StageStatus::Success(Value::Text(text)) => Ok(vec![Message {
            role: "user".to_string(),
            content: text.clone(),
        }]),
        StageStatus::Success(Value::Json(json)) => Ok(vec![Message {
            role: "user".to_string(),
            content: json.to_string(),
        }]),
        StageStatus::Invalid { errors, .. } => Err(LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!("parent stage is invalid: {}", errors.join("; ")),
        }),
        StageStatus::Skipped => Err(LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: "parent stage was skipped".to_string(),
        }),
    }
}

fn required_model(stage: &StageSpec) -> Result<&str, LlmffError> {
    stage
        .model
        .as_deref()
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: "stage requires model".to_string(),
        })
}

fn serialize_value(value: &Value) -> Result<String, LlmffError> {
    match value {
        Value::Text(text) => Ok(text.clone()),
        Value::Messages(messages) => Ok(render_messages_as_text(messages)),
        Value::Json(json) => serde_json::to_string(json).map_err(LlmffError::Json),
    }
}

fn render_messages_as_text(messages: &[Message]) -> String {
    messages
        .iter()
        .map(|message| format!("{}: {}", message.role, message.content))
        .collect::<Vec<_>>()
        .join("\n")
}

fn resolve_path(cwd: &Path, path: &str) -> PathBuf {
    let path = Path::new(path);
    resolve_path_buf(cwd, path)
}

fn resolve_path_buf(cwd: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

fn read_input(cwd: &Path, path: &str) -> Result<String, LlmffError> {
    if path == "-" {
        let mut input = String::new();
        std::io::stdin().read_to_string(&mut input)?;
        Ok(input)
    } else {
        Ok(std::fs::read_to_string(resolve_path(cwd, path))?)
    }
}

fn write_output(cwd: &Path, path: &str, value: &str) -> Result<(), LlmffError> {
    if path == "-" {
        let mut stdout = std::io::stdout().lock();
        stdout.write_all(value.as_bytes())?;
        stdout.write_all(b"\n")?;
        stdout.flush()?;
    } else {
        std::fs::write(resolve_path(cwd, path), value)?;
    }

    Ok(())
}

fn timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after Unix epoch")
        .as_millis()
}

fn status_name(status: &StageStatus) -> &'static str {
    match status {
        StageStatus::Success(_) => "success",
        StageStatus::Invalid { .. } => "invalid",
        StageStatus::Skipped => "skipped",
    }
}

fn loop_stop_reason_name(reason: &LoopStopReason) -> &'static str {
    match reason {
        LoopStopReason::BreakCondition => "break_condition",
        LoopStopReason::MaxIterations => "max_iterations",
        LoopStopReason::Error => "error",
    }
}

fn evaluate_loop_break(
    break_on: &LoopBreakSpec,
    statuses: &BTreeMap<String, StageStatus>,
) -> Result<bool, LlmffError> {
    match break_on {
        LoopBreakSpec::StageSuccess { stage } => {
            Ok(matches!(statuses.get(stage), Some(StageStatus::Success(_))))
        }
        LoopBreakSpec::StageFailure { stage } => Ok(matches!(
            statuses.get(stage),
            Some(StageStatus::Invalid { .. }) | Some(StageStatus::Skipped)
        )),
        LoopBreakSpec::FieldTrue { stage, field } => {
            let Some(value) = statuses.get(stage).and_then(success_value) else {
                return Ok(false);
            };
            Ok(json_field(&value, field)
                .and_then(|value| value.as_bool())
                .unwrap_or(false))
        }
        LoopBreakSpec::FieldEquals {
            stage,
            field,
            value,
        } => {
            let Some(actual) = statuses
                .get(stage)
                .and_then(success_value)
                .and_then(|value| json_field(&value, field).cloned())
            else {
                return Ok(false);
            };
            Ok(actual == *value)
        }
        LoopBreakSpec::Never => Ok(false),
    }
}

fn json_field<'a>(value: &'a Value, field: &str) -> Option<&'a serde_json::Value> {
    match value {
        Value::Json(json) => json.get(field),
        _ => None,
    }
}

fn reject_missing_first_iteration_carry_alias(
    loop_stage: &StageSpec,
    body_stage: &StageSpec,
    body_statuses: &BTreeMap<String, StageStatus>,
) -> Result<(), LlmffError> {
    let Some(parent) = body_stage.from.as_ref() else {
        return Ok(());
    };
    if !loop_stage.carry.contains_key(parent) || body_statuses.contains_key(parent) {
        return Ok(());
    }

    Err(LlmffError::StageExecution {
        stage_id: loop_stage.id.clone(),
        message: format!(
            "loop `{}` cannot read carry alias `{parent}` on iteration 1 before it has a previous value",
            loop_stage.id
        ),
    })
}

fn loop_final_value(
    loop_stage: &StageSpec,
    final_stage_id: &str,
    final_status: &StageStatus,
    iterations_run: usize,
    stop_reason: &LoopStopReason,
) -> Result<Value, LlmffError> {
    let require_status = loop_stage
        .final_output
        .as_ref()
        .and_then(|final_output| final_output.require_status.as_deref())
        .unwrap_or("success");
    let final_payload = match (require_status, final_status) {
        ("success", StageStatus::Success(value)) => serialize_value_to_json(value)?,
        ("invalid", StageStatus::Invalid { value, .. }) => serialize_value_to_json(value)?,
        ("any", StageStatus::Success(value)) => serialize_value_to_json(value)?,
        ("any", StageStatus::Invalid { value, .. }) => serialize_value_to_json(value)?,
        ("any", StageStatus::Skipped) => serde_json::Value::Null,
        ("success", StageStatus::Invalid { errors, .. }) => {
            return Err(LlmffError::StageExecution {
                stage_id: loop_stage.id.clone(),
                message: format!(
                    "loop final stage `{final_stage_id}` was invalid: {}",
                    errors.join("; ")
                ),
            });
        }
        ("success", StageStatus::Skipped) => {
            return Err(LlmffError::StageExecution {
                stage_id: loop_stage.id.clone(),
                message: format!("loop final stage `{final_stage_id}` was skipped"),
            });
        }
        ("invalid", StageStatus::Success(_)) | ("invalid", StageStatus::Skipped) => {
            return Err(LlmffError::StageExecution {
                stage_id: loop_stage.id.clone(),
                message: format!("loop final stage `{final_stage_id}` did not finish invalid"),
            });
        }
        _ => serde_json::Value::Null,
    };

    Ok(Value::Json(serde_json::json!({
        "final": final_payload,
        "metadata": {
            "iterations_run": iterations_run,
            "stop_reason": loop_stop_reason_name(stop_reason),
            "final_stage": final_stage_id
        }
    })))
}

fn serialize_value_to_json(value: &Value) -> Result<serde_json::Value, LlmffError> {
    match value {
        Value::Text(text) => Ok(serde_json::Value::String(text.clone())),
        Value::Messages(messages) => {
            Ok(serde_json::Value::String(render_messages_as_text(messages)))
        }
        Value::Json(json) => Ok(json.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use tempfile::tempdir;

    use crate::backend::{InferResponse, MockBackend, UsageMetadata};
    use crate::manifest::Manifest;
    use wiremock::matchers::{body_string, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[derive(Debug)]
    struct UsageBackend {
        model: String,
        text: String,
        usage: UsageMetadata,
    }

    #[async_trait::async_trait]
    impl Backend for UsageBackend {
        async fn infer(&self, request: InferRequest) -> Result<InferResponse, LlmffError> {
            assert_eq!(request.model, self.model);
            Ok(InferResponse {
                model: request.model,
                text: self.text.clone(),
                usage: Some(self.usage.clone()),
            })
        }
    }

    #[derive(Debug)]
    struct RecordingBackend {
        model: String,
        messages: Arc<Mutex<Vec<Message>>>,
        seed: Arc<Mutex<Option<u64>>>,
        response_format: Arc<Mutex<Option<String>>>,
        stop: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl Backend for RecordingBackend {
        async fn infer(&self, request: InferRequest) -> Result<InferResponse, LlmffError> {
            assert_eq!(request.model, self.model);
            *self.messages.lock().unwrap() = request.messages;
            *self.seed.lock().unwrap() = request.seed;
            *self.response_format.lock().unwrap() = request.response_format;
            *self.stop.lock().unwrap() = request.stop;
            Ok(InferResponse {
                model: request.model,
                text: "ok".to_string(),
                usage: None,
            })
        }
    }

    #[derive(Debug)]
    struct DelayedBackend {
        active: Arc<AtomicUsize>,
        max_active: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl Backend for DelayedBackend {
        async fn infer(&self, request: InferRequest) -> Result<InferResponse, LlmffError> {
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.max_active.fetch_max(active, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(25)).await;
            self.active.fetch_sub(1, Ordering::SeqCst);

            Ok(InferResponse {
                model: request.model.clone(),
                text: request.model,
                usage: None,
            })
        }
    }

    #[derive(Debug)]
    struct FlakyBackend {
        attempts: Arc<AtomicUsize>,
        failures_before_success: usize,
    }

    #[async_trait::async_trait]
    impl Backend for FlakyBackend {
        async fn infer(&self, request: InferRequest) -> Result<InferResponse, LlmffError> {
            let attempt = self.attempts.fetch_add(1, Ordering::SeqCst) + 1;
            if attempt <= self.failures_before_success {
                return Err(LlmffError::Backend("temporary model failure".to_string()));
            }

            Ok(InferResponse {
                model: request.model,
                text: "retried".to_string(),
                usage: None,
            })
        }
    }

    #[derive(Debug)]
    struct SlowBackend;

    #[async_trait::async_trait]
    impl Backend for SlowBackend {
        async fn infer(&self, request: InferRequest) -> Result<InferResponse, LlmffError> {
            tokio::time::sleep(Duration::from_millis(50)).await;
            Ok(InferResponse {
                model: request.model,
                text: "too late".to_string(),
                usage: None,
            })
        }
    }

    #[test]
    fn validate_manifest_rejects_unknown_stage_operation() {
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
graph:
  - id: mystery
    op: unknown_op
"#,
        )
        .unwrap();

        let error = Engine::new()
            .validate_manifest(manifest)
            .expect_err("unknown stage operation should be rejected");

        assert!(error
            .to_string()
            .contains("unknown stage operation `unknown_op`"));
    }

    #[test]
    fn validate_manifest_rejects_missing_required_stage_parameters() {
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
inputs:
  prompt:
    path: prompt.txt
graph:
  - id: prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: prompt
  - id: validate
    op: validate_json
    from: draft
"#,
        )
        .unwrap();

        let error = Engine::new()
            .validate_manifest(manifest)
            .expect_err("missing infer model should be rejected first");

        assert!(error
            .to_string()
            .contains("stage `draft` failed: infer requires model"));
    }

    #[test]
    fn validate_manifest_rejects_missing_backend_without_calling_it() {
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
inputs:
  prompt:
    path: prompt.txt
graph:
  - id: prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: prompt
    model: openai:gpt-test
"#,
        )
        .unwrap();

        let error = Engine::new()
            .validate_manifest(manifest)
            .expect_err("unregistered backend alias should be rejected");

        assert!(error
            .to_string()
            .contains("no backend configured for `openai:gpt-test`"));
    }

    #[test]
    fn validate_manifest_rejects_retrieve_without_parent() {
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
graph:
  - id: retrieve_context
    op: retrieve
    documents: [docs/rust.txt]
"#,
        )
        .unwrap();

        let error = Engine::new()
            .validate_manifest(manifest)
            .expect_err("retrieve without parent should be rejected");

        assert!(error
            .to_string()
            .contains("stage `retrieve_context` failed: retrieve requires from"));
    }

    #[test]
    fn validate_manifest_rejects_retrieve_without_documents() {
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
inputs:
  prompt:
    path: prompt.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: retrieve_context
    op: retrieve
    from: load_prompt
"#,
        )
        .unwrap();

        let error = Engine::new()
            .validate_manifest(manifest)
            .expect_err("retrieve without documents should be rejected");

        assert!(error
            .to_string()
            .contains("stage `retrieve_context` failed: retrieve requires documents"));
    }

    #[test]
    fn validate_manifest_rejects_unknown_retrieve_strategy() {
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
inputs:
  prompt:
    path: question.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: retrieve_context
    op: retrieve
    from: load_prompt
    documents: [docs/rust.txt]
    strategy: remote
"#,
        )
        .unwrap();

        let error = Engine::new()
            .validate_manifest(manifest)
            .expect_err("unknown retrieve strategy should be rejected");

        assert!(error
            .to_string()
            .contains("retrieve strategy must be lexical, embedding, or command"));
    }

    #[test]
    fn validate_manifest_rejects_retrieve_index_for_command_strategy() {
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
inputs:
  prompt:
    path: question.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: retrieve_context
    op: retrieve
    from: load_prompt
    documents: [docs/rust.txt]
    strategy: command
    command: ["/bin/cat"]
    index: .llmff/retrieve/context.index.json
"#,
        )
        .unwrap();

        let error = Engine::new()
            .validate_manifest(manifest)
            .expect_err("command retrieve index should be rejected");

        assert!(error
            .to_string()
            .contains("retrieve index requires embedding strategy"));
    }

    #[test]
    fn validate_manifest_rejects_unknown_rerank_strategy() {
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
inputs:
  candidates:
    path: matches.json
    format: json
graph:
  - id: load_candidates
    op: load
    input: candidates
  - id: rerank_context
    op: rerank
    from: load_candidates
    strategy: remote
"#,
        )
        .unwrap();

        let error = Engine::new()
            .validate_manifest(manifest)
            .expect_err("unknown rerank strategy should be rejected");

        assert!(error
            .to_string()
            .contains("rerank strategy must be lexical, embedding, or command"));
    }

    #[test]
    fn validate_manifest_rejects_loop_without_from() {
        let engine = Engine::new().with_backend(
            "mock:json",
            Arc::new(MockBackend::new("mock:json", r#"{"answer":"ok"}"#)),
        );
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
graph:
  - id: refine
    op: loop
    max_iterations: 2
    break_on: { type: never }
    body:
      - id: draft
        op: infer
        from: input
        model: mock:json
"#,
        )
        .unwrap();

        let error = engine.validate_manifest(manifest).unwrap_err().to_string();
        assert!(error.contains("loop requires from"));
    }

    #[test]
    fn validate_manifest_rejects_invalid_loop_error_policy() {
        let engine = Engine::new().with_backend(
            "mock:json",
            Arc::new(MockBackend::new("mock:json", r#"{"answer":"ok"}"#)),
        );
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
inputs:
  prompt:
    path: prompt.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: refine
    op: loop
    from: load_prompt
    max_iterations: 2
    on_iteration_error: retry_forever
    break_on: { type: never }
    body:
      - id: draft
        op: infer
        from: input
        model: mock:json
"#,
        )
        .unwrap();

        let error = engine.validate_manifest(manifest).unwrap_err().to_string();
        assert!(error.contains("on_iteration_error must be fail, continue, or break"));
    }

    #[test]
    fn validate_manifest_accepts_valid_loop_body() {
        let engine = Engine::new().with_backend(
            "mock:json",
            Arc::new(MockBackend::new("mock:json", r#"{"answer":"ok"}"#)),
        );
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
inputs:
  prompt:
    path: prompt.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: refine
    op: loop
    from: load_prompt
    max_iterations: 2
    break_on: { type: never }
    body:
      - id: draft
        op: infer
        from: input
        model: mock:json
      - id: check
        op: validate_json
        from: draft
        schema: '{"type":"object","required":["answer"]}'
"#,
        )
        .unwrap();

        engine
            .validate_manifest(manifest)
            .expect("valid loop body should pass validation");
    }

    #[tokio::test]
    async fn loop_runs_until_stage_success_break_condition() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("prompt.txt"), "question").unwrap();
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
inputs:
  prompt:
    path: prompt.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: refine
    op: loop
    from: load_prompt
    max_iterations: 3
    break_on: { type: stage_success, stage: check }
    final: { from: draft, require_status: success }
    body:
      - id: draft
        op: infer
        from: input
        model: mock:json
      - id: check
        op: validate_json
        from: draft
        schema: '{"type":"object","required":["answer"]}'
outputs:
  final:
    from: refine
    path: answer.json
"#,
        )
        .unwrap();
        let engine = Engine::new().with_backend(
            "mock:json",
            Arc::new(MockBackend::new("mock:json", r#"{"answer":"ok"}"#)),
        );

        engine.run_manifest(manifest, dir.path()).await.unwrap();

        let output = std::fs::read_to_string(dir.path().join("answer.json")).unwrap();
        let output: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(output["metadata"]["iterations_run"], 1);
        assert_eq!(output["metadata"]["stop_reason"], "break_condition");
        assert_eq!(output["final"], r#"{"answer":"ok"}"#);
    }

    #[tokio::test]
    async fn loop_never_breaks_until_max_iterations() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("prompt.txt"), "question").unwrap();
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
inputs:
  prompt:
    path: prompt.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: sample
    op: loop
    from: load_prompt
    max_iterations: 2
    break_on: { type: never }
    final: { from: draft, require_status: success }
    body:
      - id: draft
        op: infer
        from: input
        model: mock:json
outputs:
  final:
    from: sample
    path: answer.json
"#,
        )
        .unwrap();
        let engine = Engine::new().with_backend(
            "mock:json",
            Arc::new(MockBackend::new("mock:json", r#"{"answer":"ok"}"#)),
        );

        engine.run_manifest(manifest, dir.path()).await.unwrap();

        let output = std::fs::read_to_string(dir.path().join("answer.json")).unwrap();
        let output: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(output["metadata"]["iterations_run"], 2);
        assert_eq!(output["metadata"]["stop_reason"], "max_iterations");
    }

    #[tokio::test]
    async fn loop_breaks_on_iteration_error_and_preserves_text_payload() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("prompt.txt"), "question").unwrap();
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
inputs:
  prompt:
    path: prompt.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: sample
    op: loop
    from: load_prompt
    max_iterations: 2
    break_on: { type: never }
    on_iteration_error: break
    final: { from: draft, require_status: success }
    body:
      - id: draft
        op: infer
        from: input
        model: mock:text
      - id: explode
        op: tool
        from: draft
        command: ["/bin/sh", "-c", "exit 9"]
outputs:
  final:
    from: sample
    path: answer.json
"#,
        )
        .unwrap();
        let engine = Engine::new()
            .with_backend("mock:text", Arc::new(MockBackend::new("mock:text", "true")));

        engine.run_manifest(manifest, dir.path()).await.unwrap();

        let output = std::fs::read_to_string(dir.path().join("answer.json")).unwrap();
        let output: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(output["metadata"]["iterations_run"], 1);
        assert_eq!(output["metadata"]["stop_reason"], "error");
        assert_eq!(output["final"], "true");
    }

    #[tokio::test]
    async fn loop_reports_iteration_one_carry_alias_without_previous_value() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("prompt.txt"), "question").unwrap();
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
inputs:
  prompt:
    path: prompt.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: refine
    op: loop
    from: load_prompt
    max_iterations: 2
    break_on: { type: never }
    carry:
      previous: draft
    final: { from: draft, require_status: success }
    body:
      - id: draft
        op: infer
        from: previous
        model: mock:text
"#,
        )
        .unwrap();
        let engine = Engine::new().with_backend(
            "mock:text",
            Arc::new(MockBackend::new("mock:text", "answer")),
        );

        let error = engine
            .run_manifest(manifest, dir.path())
            .await
            .expect_err("iteration 1 carry alias should fail clearly");

        let message = error.to_string();
        assert!(message.contains("loop `refine`"));
        assert!(message.contains("carry alias `previous`"));
        assert!(message.contains("iteration 1"));
    }

    #[tokio::test]
    async fn cache_stage_writes_and_reuses_success_value() {
        let dir = tempdir().unwrap();
        let manifest = cache_manifest("answer-v1");
        let prompt_path = dir.path().join("prompt.txt");
        let output_path = dir.path().join("answer.txt");
        let engine = Engine::new();

        std::fs::write(&prompt_path, "first").unwrap();
        engine
            .run_manifest(manifest.clone(), dir.path())
            .await
            .expect("first run should populate cache");
        assert_eq!(std::fs::read_to_string(&output_path).unwrap(), "first");

        std::fs::write(&prompt_path, "second").unwrap();
        engine
            .run_manifest(manifest, dir.path())
            .await
            .expect("second run should read cache");
        assert_eq!(std::fs::read_to_string(&output_path).unwrap(), "first");
    }

    #[tokio::test]
    async fn cache_stage_refresh_policy_replaces_existing_value() {
        let dir = tempdir().unwrap();
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
inputs:
  prompt:
    path: prompt.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: cached
    op: cache
    from: load_prompt
    path: .llmff/cache
    key: answer-v1
    cache_policy: refresh
outputs:
  final:
    from: cached
    path: answer.txt
"#,
        )
        .unwrap();
        let prompt_path = dir.path().join("prompt.txt");
        let output_path = dir.path().join("answer.txt");

        std::fs::write(&prompt_path, "first").unwrap();
        Engine::new()
            .run_manifest(manifest.clone(), dir.path())
            .await
            .expect("first run should populate cache");
        std::fs::write(&prompt_path, "second").unwrap();
        Engine::new()
            .run_manifest(manifest, dir.path())
            .await
            .expect("refresh run should replace cache");

        assert_eq!(std::fs::read_to_string(output_path).unwrap(), "second");
    }

    #[tokio::test]
    async fn model_stage_retries_before_failing() {
        let dir = tempdir().unwrap();
        let prompt_path = dir.path().join("prompt.txt");
        let output_path = dir.path().join("answer.txt");
        std::fs::write(&prompt_path, "hello").unwrap();
        let attempts = Arc::new(AtomicUsize::new(0));
        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: load_prompt
    model: flaky
    retry:
      attempts: 3
      backoff_ms: 0
outputs:
  final:
    from: draft
    path: {}
"#,
            prompt_path.display(),
            output_path.display()
        ))
        .unwrap();
        let engine = Engine::new().with_backend(
            "flaky",
            Arc::new(FlakyBackend {
                attempts: attempts.clone(),
                failures_before_success: 2,
            }),
        );

        engine
            .run_manifest(manifest, dir.path())
            .await
            .expect("third model attempt should succeed");

        assert_eq!(attempts.load(Ordering::SeqCst), 3);
        assert_eq!(std::fs::read_to_string(output_path).unwrap(), "retried");
    }

    #[tokio::test]
    async fn trace_events_include_retry_attempts() {
        let dir = tempdir().unwrap();
        let prompt_path = dir.path().join("prompt.txt");
        let output_path = dir.path().join("answer.txt");
        let trace_path = dir.path().join("trace.jsonl");
        std::fs::write(&prompt_path, "hello").unwrap();
        let attempts = Arc::new(AtomicUsize::new(0));
        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: load_prompt
    model: flaky
    retry:
      attempts: 3
      backoff_ms: 0
outputs:
  final:
    from: draft
    path: {}
"#,
            prompt_path.display(),
            output_path.display()
        ))
        .unwrap();
        let engine = Engine::new().with_backend(
            "flaky",
            Arc::new(FlakyBackend {
                attempts: attempts.clone(),
                failures_before_success: 2,
            }),
        );

        engine
            .run_manifest_with_options(
                manifest,
                dir.path(),
                RunOptions {
                    trace_path: Some(trace_path.clone()),
                    ..RunOptions::default()
                },
            )
            .await
            .expect("third model attempt should succeed");

        let trace = std::fs::read_to_string(trace_path).unwrap();
        let events = parse_trace_events(&trace);
        let draft_finished = trace_stage_finished(&events, "draft");
        assert_eq!(draft_finished["attempts"], 3);
    }

    #[tokio::test]
    async fn loop_body_trace_records_include_iteration_context() {
        let dir = tempfile::tempdir().unwrap();
        let trace_path = dir.path().join("trace.jsonl");
        std::fs::write(dir.path().join("prompt.txt"), "question").unwrap();
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
inputs:
  prompt:
    path: prompt.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: sample
    op: loop
    from: load_prompt
    max_iterations: 2
    break_on: { type: never }
    final: { from: draft, require_status: success }
    body:
      - id: draft
        op: infer
        from: input
        model: mock:json
"#,
        )
        .unwrap();
        let engine = Engine::new().with_backend(
            "mock:json",
            Arc::new(MockBackend::new("mock:json", r#"{"answer":"ok"}"#)),
        );

        engine
            .run_manifest_with_options(
                manifest,
                dir.path(),
                RunOptions {
                    run_id: "loop-trace".to_string(),
                    trace_path: Some(trace_path.clone()),
                    ..RunOptions::default()
                },
            )
            .await
            .unwrap();

        let trace = std::fs::read_to_string(trace_path).unwrap();
        let events = parse_trace_events(&trace);
        let draft_events = events
            .iter()
            .filter(|event| event["stage_id"] == "sample.draft")
            .collect::<Vec<_>>();
        assert!(draft_events
            .iter()
            .any(|event| event["loop_id"] == "sample" && event["loop_iteration"] == 1));
        assert!(draft_events
            .iter()
            .any(|event| event["loop_id"] == "sample" && event["loop_iteration"] == 2));
        assert!(draft_events
            .iter()
            .all(|event| event["loop_stage_id"] == "draft"));
    }

    #[tokio::test]
    async fn parallel_loop_body_trace_records_include_iteration_context() {
        let dir = tempfile::tempdir().unwrap();
        let trace_path = dir.path().join("trace.jsonl");
        std::fs::write(dir.path().join("prompt.txt"), "question").unwrap();
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
inputs:
  prompt:
    path: prompt.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: sample
    op: loop
    from: load_prompt
    max_iterations: 2
    break_on: { type: never }
    final: { from: draft, require_status: success }
    body:
      - id: draft
        op: infer
        from: input
        model: mock:json
"#,
        )
        .unwrap();
        let engine = Engine::new().with_backend(
            "mock:json",
            Arc::new(MockBackend::new("mock:json", r#"{"answer":"ok"}"#)),
        );

        engine
            .run_manifest_with_options(
                manifest,
                dir.path(),
                RunOptions {
                    run_id: "parallel-loop-trace".to_string(),
                    trace_path: Some(trace_path.clone()),
                    scheduler: SchedulerMode::Parallel,
                    ..RunOptions::default()
                },
            )
            .await
            .unwrap();

        let trace = std::fs::read_to_string(trace_path).unwrap();
        let events = parse_trace_events(&trace);
        let draft_events = events
            .iter()
            .filter(|event| event["stage_id"] == "sample.draft")
            .collect::<Vec<_>>();
        assert!(draft_events
            .iter()
            .any(|event| event["loop_id"] == "sample" && event["loop_iteration"] == 1));
        assert!(draft_events
            .iter()
            .any(|event| event["loop_id"] == "sample" && event["loop_iteration"] == 2));
        assert!(draft_events
            .iter()
            .all(|event| event["loop_stage_id"] == "draft"));
    }

    #[tokio::test]
    async fn loop_body_failure_trace_includes_iteration_context() {
        let dir = tempfile::tempdir().unwrap();
        let trace_path = dir.path().join("trace.jsonl");
        std::fs::write(dir.path().join("prompt.txt"), "question").unwrap();
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
inputs:
  prompt:
    path: prompt.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: sample
    op: loop
    from: load_prompt
    max_iterations: 2
    break_on: { type: never }
    final: { from: draft, require_status: success }
    body:
      - id: draft
        op: load
        input: missing_prompt
"#,
        )
        .unwrap();

        Engine::new()
            .run_manifest_with_options(
                manifest,
                dir.path(),
                RunOptions {
                    run_id: "loop-failure-trace".to_string(),
                    trace_path: Some(trace_path.clone()),
                    ..RunOptions::default()
                },
            )
            .await
            .expect_err("loop body stage should fail");

        let trace = std::fs::read_to_string(trace_path).unwrap();
        let events = parse_trace_events(&trace);
        let failed = events
            .iter()
            .find(|event| event["event"] == "run_failed")
            .expect("run_failed event should exist");

        assert_eq!(failed["stage_id"], "sample.draft");
        assert_eq!(failed["loop_id"], "sample");
        assert_eq!(failed["loop_iteration"], 1);
        assert_eq!(failed["loop_stage_id"], "draft");
    }

    #[tokio::test]
    async fn loop_body_nonfatal_error_trace_records_terminal_event() {
        let dir = tempfile::tempdir().unwrap();
        let trace_path = dir.path().join("trace.jsonl");
        std::fs::write(dir.path().join("prompt.txt"), "question").unwrap();
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
inputs:
  prompt:
    path: prompt.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: sample
    op: loop
    from: load_prompt
    max_iterations: 2
    break_on: { type: never }
    on_iteration_error: break
    final: { from: draft, require_status: success }
    body:
      - id: draft
        op: infer
        from: input
        model: mock:text
      - id: explode
        op: tool
        from: draft
        command: ["/bin/sh", "-c", "exit 9"]
"#,
        )
        .unwrap();
        let engine = Engine::new()
            .with_backend("mock:text", Arc::new(MockBackend::new("mock:text", "true")));

        engine
            .run_manifest_with_options(
                manifest,
                dir.path(),
                RunOptions {
                    run_id: "loop-nonfatal-trace".to_string(),
                    trace_path: Some(trace_path.clone()),
                    ..RunOptions::default()
                },
            )
            .await
            .unwrap();

        let trace = std::fs::read_to_string(trace_path).unwrap();
        let events = parse_trace_events(&trace);
        assert!(events.iter().all(|event| event["event"] != "run_failed"));
        let explode_events = events
            .iter()
            .filter(|event| event["stage_id"] == "sample.explode")
            .collect::<Vec<_>>();
        assert_eq!(
            explode_events
                .iter()
                .filter(|event| event["event"] == "stage_started")
                .count(),
            1
        );
        let terminal_events = explode_events
            .iter()
            .filter(|event| event["event"] == "stage_finished")
            .collect::<Vec<_>>();
        assert_eq!(terminal_events.len(), 1);
        let terminal = terminal_events[0];
        assert_eq!(terminal["status"], "error");
        assert_eq!(terminal["loop_id"], "sample");
        assert_eq!(terminal["loop_iteration"], 1);
        assert_eq!(terminal["loop_stage_id"], "explode");
    }

    #[tokio::test]
    async fn http_tool_stage_retries_server_failures() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/tool"))
            .respond_with(ResponseTemplate::new(500).set_body_string("temporary"))
            .up_to_n_times(2)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/tool"))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
            .mount(&server)
            .await;
        let dir = tempdir().unwrap();
        let input_path = dir.path().join("input.txt");
        let output_path = dir.path().join("answer.txt");
        let trace_path = dir.path().join("trace.jsonl");
        std::fs::write(&input_path, "hello").unwrap();
        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: call_tool
    op: tool
    from: load_prompt
    method: POST
    url: {}/tool
    retry:
      attempts: 3
      backoff_ms: 0
outputs:
  final:
    from: call_tool
    path: {}
"#,
            input_path.display(),
            server.uri(),
            output_path.display()
        ))
        .unwrap();

        Engine::new()
            .run_manifest_with_options(
                manifest,
                dir.path(),
                RunOptions {
                    trace_path: Some(trace_path.clone()),
                    ..RunOptions::default()
                },
            )
            .await
            .expect("third HTTP attempt should succeed");

        assert_eq!(std::fs::read_to_string(output_path).unwrap(), "ok");
        let trace = std::fs::read_to_string(trace_path).unwrap();
        let events = parse_trace_events(&trace);
        let call_tool_finished = trace_stage_finished(&events, "call_tool");
        assert_eq!(call_tool_finished["attempts"], 3);
    }

    #[tokio::test]
    async fn http_tool_stage_does_not_retry_client_failures() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/tool"))
            .respond_with(ResponseTemplate::new(400).set_body_string("bad request"))
            .expect(1)
            .mount(&server)
            .await;
        let dir = tempdir().unwrap();
        let input_path = dir.path().join("input.txt");
        std::fs::write(&input_path, "hello").unwrap();
        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: call_tool
    op: tool
    from: load_prompt
    method: POST
    url: {}/tool
    retry:
      attempts: 3
      backoff_ms: 0
"#,
            input_path.display(),
            server.uri()
        ))
        .unwrap();

        let error = Engine::new()
            .run_manifest(manifest, dir.path())
            .await
            .expect_err("client HTTP failure should not retry");

        assert!(error.to_string().contains("http tool returned status 400"));
    }

    #[tokio::test]
    async fn stage_timeout_fails_with_safe_failure_kind() {
        let dir = tempdir().unwrap();
        let prompt_path = dir.path().join("prompt.txt");
        let trace_path = dir.path().join("trace.jsonl");
        std::fs::write(&prompt_path, "hello").unwrap();
        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: load_prompt
    model: slow
    timeout_ms: 1
outputs:
  final:
    from: draft
    path: answer.txt
"#,
            prompt_path.display()
        ))
        .unwrap();
        let options = RunOptions {
            run_id: "timeout-run".to_string(),
            trace_path: Some(trace_path.clone()),
            ..RunOptions::default()
        };

        let error = Engine::new()
            .with_backend("slow", Arc::new(SlowBackend))
            .run_manifest_with_options(manifest, dir.path(), options)
            .await
            .expect_err("stage should time out");

        assert!(error.to_string().contains("stage timed out"));
        let events = parse_trace_events(&std::fs::read_to_string(trace_path).unwrap());
        let failed = events
            .iter()
            .find(|event| event["event"] == "run_failed")
            .expect("run_failed event should exist");
        assert_eq!(failed["failure_kind"], "timeout");
        assert_eq!(failed["failure_message"], "stage timed out");
    }

    #[tokio::test]
    async fn command_tool_timeout_preempts_blocking_process() {
        let dir = tempdir().unwrap();
        let input_path = dir.path().join("input.txt");
        std::fs::write(&input_path, "hello").unwrap();
        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: slow_tool
    op: tool
    from: load_prompt
    command: ["sh", "-c", "sleep 2"]
    timeout_ms: 10
outputs:
  final:
    from: slow_tool
    path: answer.txt
"#,
            input_path.display()
        ))
        .unwrap();
        let started = Instant::now();

        let error = Engine::new()
            .run_manifest(manifest, dir.path())
            .await
            .expect_err("slow command should time out");

        assert!(error.to_string().contains("stage timed out"));
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "timeout should preempt the process"
        );
    }

    #[tokio::test]
    async fn parallel_scheduler_respects_max_concurrency() {
        let (manifest, dir, active, max_active) = parallel_scheduler_fixture();

        Engine::new()
            .with_backend(
                "delay",
                Arc::new(DelayedBackend {
                    active,
                    max_active: max_active.clone(),
                }),
            )
            .run_manifest_with_options(
                manifest,
                dir.path(),
                RunOptions {
                    scheduler: SchedulerMode::Parallel,
                    max_concurrency: Some(1),
                    ..RunOptions::default()
                },
            )
            .await
            .expect("parallel run should succeed");

        assert_eq!(max_active.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn checkpoint_resume_skips_completed_stages() {
        let dir = tempdir().unwrap();
        let prompt_path = dir.path().join("prompt.txt");
        let output_path = dir.path().join("answer.txt");
        let checkpoint_path = dir.path().join("checkpoint.json");
        std::fs::write(&prompt_path, "first").unwrap();
        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: cached
    op: cache
    from: load_prompt
    path: .llmff/cache
    key: answer-v1
outputs:
  final:
    from: cached
    path: {}
"#,
            prompt_path.display(),
            output_path.display()
        ))
        .unwrap();

        Engine::new()
            .run_manifest_with_options(
                manifest.clone(),
                dir.path(),
                RunOptions {
                    checkpoint_path: Some(checkpoint_path.clone()),
                    ..RunOptions::default()
                },
            )
            .await
            .expect("first run should write checkpoint");
        std::fs::write(&prompt_path, "second").unwrap();
        Engine::new()
            .run_manifest_with_options(
                manifest,
                dir.path(),
                RunOptions {
                    resume_path: Some(checkpoint_path.clone()),
                    ..RunOptions::default()
                },
            )
            .await
            .expect("resume should reuse completed statuses");

        assert_eq!(std::fs::read_to_string(output_path).unwrap(), "first");
    }

    #[tokio::test]
    async fn checkpoint_resume_rejects_mismatched_manifest() {
        let dir = tempdir().unwrap();
        let prompt_path = dir.path().join("prompt.txt");
        let checkpoint_path = dir.path().join("checkpoint.json");
        std::fs::write(&prompt_path, "first").unwrap();
        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
outputs:
  final:
    from: load_prompt
    path: answer.txt
"#,
            prompt_path.display()
        ))
        .unwrap();

        Engine::new()
            .run_manifest_with_options(
                manifest.clone(),
                dir.path(),
                RunOptions {
                    checkpoint_path: Some(checkpoint_path.clone()),
                    ..RunOptions::default()
                },
            )
            .await
            .expect("first run should write checkpoint");
        let mut changed = manifest;
        changed.outputs.get_mut("final").unwrap().path = "other.txt".to_string();

        let error = Engine::new()
            .run_manifest_with_options(
                changed,
                dir.path(),
                RunOptions {
                    resume_path: Some(checkpoint_path.clone()),
                    ..RunOptions::default()
                },
            )
            .await
            .expect_err("mismatched manifest should reject checkpoint");

        assert!(error
            .to_string()
            .contains("checkpoint manifest hash does not match"));
        assert!(error
            .to_string()
            .contains(checkpoint_path.to_str().unwrap()));
        assert!(error.to_string().contains("checkpoint_hash="));
        assert!(error.to_string().contains("current_manifest_hash="));
        assert!(error.to_string().contains("run inspect --format json"));
    }

    #[tokio::test]
    async fn replay_trace_requires_checkpoint_payloads() {
        let dir = tempdir().unwrap();
        let trace_path = dir.path().join("trace.jsonl");
        std::fs::write(
            &trace_path,
            r#"{"run_id":"test","event":"stage_finished","stage_id":"load_prompt","op":"load","status":"success","timestamp_ms":1}"#,
        )
        .unwrap();
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
graph: []
"#,
        )
        .unwrap();

        let error = Engine::new()
            .run_manifest_with_options(
                manifest,
                dir.path(),
                RunOptions {
                    replay_trace_path: Some(trace_path),
                    ..RunOptions::default()
                },
            )
            .await
            .expect_err("trace-only replay should be rejected safely");

        assert!(error
            .to_string()
            .contains("trace replay requires a checkpoint"));
    }

    #[tokio::test]
    async fn replay_trace_with_matching_checkpoint_reuses_completed_stages() {
        let dir = tempdir().unwrap();
        let prompt_path = dir.path().join("prompt.txt");
        let output_path = dir.path().join("answer.txt");
        let checkpoint_path = dir.path().join("checkpoint.json");
        let trace_path = dir.path().join("trace.jsonl");
        std::fs::write(&prompt_path, "first").unwrap();
        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
outputs:
  final:
    from: load_prompt
    path: {}
"#,
            prompt_path.display(),
            output_path.display()
        ))
        .unwrap();

        Engine::new()
            .run_manifest_with_options(
                manifest.clone(),
                dir.path(),
                RunOptions {
                    checkpoint_path: Some(checkpoint_path.clone()),
                    trace_path: Some(trace_path.clone()),
                    ..RunOptions::default()
                },
            )
            .await
            .expect("first run should write checkpoint and trace");
        std::fs::write(&prompt_path, "second").unwrap();
        Engine::new()
            .run_manifest_with_options(
                manifest,
                dir.path(),
                RunOptions {
                    resume_path: Some(checkpoint_path),
                    replay_trace_path: Some(trace_path),
                    ..RunOptions::default()
                },
            )
            .await
            .expect("checkpoint-backed replay should reuse completed status");

        assert_eq!(std::fs::read_to_string(output_path).unwrap(), "first");
    }

    #[test]
    fn validate_manifest_rejects_cache_without_parent() {
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
graph:
  - id: cached
    op: cache
"#,
        )
        .unwrap();

        let error = Engine::new()
            .validate_manifest(manifest)
            .expect_err("cache without parent should be rejected");

        assert!(error
            .to_string()
            .contains("stage `cached` failed: cache requires from"));
    }

    #[test]
    fn validate_manifest_rejects_unknown_when_condition() {
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
inputs:
  prompt:
    path: prompt.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: load_prompt
    when: maybe
    model: mock:good
"#,
        )
        .unwrap();

        let error = Engine::new()
            .with_backend("mock:good", Arc::new(MockBackend::new("mock:good", "ok")))
            .validate_manifest(manifest)
            .expect_err("unknown when condition should be rejected");

        assert!(error
            .to_string()
            .contains("stage `draft` failed: unknown when condition `maybe`"));
    }

    #[test]
    fn validate_manifest_rejects_unknown_input_format() {
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
inputs:
  payload:
    path: payload.json
    format: yaml
graph:
  - id: load_payload
    op: load
    input: payload
"#,
        )
        .unwrap();

        let error = Engine::new()
            .validate_manifest(manifest)
            .expect_err("unknown input format should be rejected");

        assert!(error
            .to_string()
            .contains("input `payload` has unsupported format `yaml`"));
    }

    #[test]
    fn validate_manifest_rejects_invalid_sampling_parameters() {
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
inputs:
  prompt:
    path: prompt.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: load_prompt
    model: mock:good
    top_p: 1.5
"#,
        )
        .unwrap();

        let error = Engine::new()
            .with_backend("mock:good", Arc::new(MockBackend::new("mock:good", "ok")))
            .validate_manifest(manifest)
            .expect_err("invalid sampling parameter should be rejected");

        assert!(error
            .to_string()
            .contains("stage `draft` failed: top_p must be between 0 and 1"));
    }

    #[test]
    fn validate_manifest_rejects_empty_stop_sequence() {
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
inputs:
  prompt:
    path: prompt.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: load_prompt
    model: mock:good
    stop: ["END", ""]
"#,
        )
        .unwrap();

        let error = Engine::new()
            .with_backend("mock:good", Arc::new(MockBackend::new("mock:good", "ok")))
            .validate_manifest(manifest)
            .expect_err("empty stop sequence should be rejected");

        assert!(error
            .to_string()
            .contains("stage `draft` failed: stop sequences cannot be empty"));
    }

    #[test]
    fn validate_manifest_rejects_unknown_response_format() {
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
inputs:
  prompt:
    path: prompt.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: load_prompt
    model: mock:good
    response_format: xml
"#,
        )
        .unwrap();

        let error = Engine::new()
            .with_backend("mock:good", Arc::new(MockBackend::new("mock:good", "ok")))
            .validate_manifest(manifest)
            .expect_err("unknown response format should be rejected");

        assert!(error
            .to_string()
            .contains("stage `draft` failed: response_format must be json"));
    }

    #[test]
    fn validate_manifest_rejects_field_route_from_text_source() {
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
inputs:
  prompt:
    path: prompt.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: fast_answer
    op: template
    from: load_prompt
    path: fast.tmpl
  - id: choose
    op: route
    from: load_prompt
    field: kind
    cases:
      simple: fast_answer
"#,
        )
        .unwrap();

        let error = Engine::new()
            .validate_manifest(manifest)
            .expect_err("field route from text source should be rejected");

        assert!(error.to_string().contains(
            "stage `choose` failed: field route requires JSON source `load_prompt`, got text"
        ));
    }

    #[test]
    fn validate_manifest_accepts_field_route_from_json_source() {
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
inputs:
  prompt:
    path: prompt.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: validate
    op: validate_json
    from: load_prompt
    schema: '{"type":"object","required":["kind"]}'
  - id: fast_answer
    op: template
    from: load_prompt
    path: fast.tmpl
  - id: choose
    op: route
    from: validate
    field: kind
    cases:
      simple: fast_answer
"#,
        )
        .unwrap();

        Engine::new()
            .validate_manifest(manifest)
            .expect("field route from validate_json should validate");
    }

    #[tokio::test]
    async fn runs_manifest_in_dependency_order() {
        let dir = tempdir().unwrap();
        let prompt_path = dir.path().join("question.txt");
        let output_path = dir.path().join("answer.txt");
        std::fs::write(&prompt_path, "Return an answer object").unwrap();

        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: write_answer
    op: write
    from: draft
    path: {}
  - id: draft
    op: infer
    from: load_prompt
    model: mock:good
  - id: load_prompt
    op: load
    input: prompt
"#,
            prompt_path.display(),
            output_path.display()
        ))
        .unwrap();

        let engine = Engine::new().with_backend(
            "mock:good",
            Arc::new(MockBackend::new("mock:good", "dependency ordered")),
        );

        let report = engine.run_manifest(manifest, dir.path()).await.unwrap();

        assert_eq!(report.final_status, RunStatus::Succeeded);
        assert_eq!(
            std::fs::read_to_string(output_path).unwrap(),
            "dependency ordered"
        );
    }

    #[tokio::test]
    async fn runs_json_repair_pipeline_end_to_end() {
        let dir = tempdir().unwrap();
        let prompt_path = dir.path().join("question.txt");
        let output_path = dir.path().join("answer.json");
        std::fs::write(&prompt_path, "Return an answer object").unwrap();

        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: load_prompt
    model: mock:bad
  - id: validate
    op: validate_json
    from: draft
    schema: '{{"type":"object","required":["answer"]}}'
  - id: repair
    op: repair
    from: validate
    when: invalid
    model: mock:good
outputs:
  final:
    from: repair
    path: {}
"#,
            prompt_path.display(),
            output_path.display()
        ))
        .unwrap();

        let engine = Engine::new()
            .with_backend(
                "mock:bad",
                Arc::new(MockBackend::new("mock:bad", r#"{"wrong":true}"#)),
            )
            .with_backend(
                "mock:good",
                Arc::new(MockBackend::new("mock:good", r#"{"answer":"ok"}"#)),
            );

        let report = engine.run_manifest(manifest, dir.path()).await.unwrap();

        assert_eq!(report.final_status, RunStatus::Succeeded);
        assert_eq!(
            std::fs::read_to_string(output_path).unwrap(),
            r#"{"answer":"ok"}"#
        );
    }

    #[tokio::test]
    async fn load_stage_reads_json_input_format() {
        let dir = tempdir().unwrap();
        let payload_path = dir.path().join("payload.json");
        let template_path = dir.path().join("simple.tmpl");
        let output_path = dir.path().join("selected.txt");
        std::fs::write(&payload_path, r#"{"kind":"simple","answer":"ok"}"#).unwrap();
        std::fs::write(&template_path, "{{answer}}").unwrap();

        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  payload:
    path: {}
    format: json
graph:
  - id: load_payload
    op: load
    input: payload
  - id: simple_answer
    op: template
    from: load_payload
    path: {}
outputs:
  final:
    from: simple_answer
    path: {}
"#,
            payload_path.display(),
            template_path.display(),
            output_path.display()
        ))
        .unwrap();

        let report = Engine::new()
            .run_manifest(manifest, dir.path())
            .await
            .unwrap();

        assert_eq!(report.final_status, RunStatus::Succeeded);
        assert_eq!(std::fs::read_to_string(output_path).unwrap(), "ok");
    }

    #[tokio::test]
    async fn when_invalid_skips_stage_on_success_parent() {
        let dir = tempdir().unwrap();
        let prompt_path = dir.path().join("question.txt");
        let output_path = dir.path().join("answer.json");
        std::fs::write(&prompt_path, "Return an answer object").unwrap();

        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: load_prompt
    model: mock:good
  - id: validate
    op: validate_json
    from: draft
    schema: '{{"type":"object","required":["answer"]}}'
  - id: repair
    op: repair
    from: validate
    when: invalid
    model: mock:repair
outputs:
  final:
    from: repair
    path: {}
"#,
            prompt_path.display(),
            output_path.display()
        ))
        .unwrap();

        let engine = Engine::new()
            .with_backend(
                "mock:good",
                Arc::new(MockBackend::new("mock:good", r#"{"answer":"ok"}"#)),
            )
            .with_backend(
                "mock:repair",
                Arc::new(MockBackend::new("mock:repair", r#"{"answer":"repaired"}"#)),
            );

        let error = engine
            .run_manifest(manifest, dir.path())
            .await
            .expect_err("output from skipped repair stage should fail");

        assert!(error
            .to_string()
            .contains("stage `repair` failed: output references skipped stage"));
        assert!(!output_path.exists());
    }

    #[tokio::test]
    async fn when_skipped_runs_stage_on_skipped_parent() {
        let dir = tempdir().unwrap();
        let prompt_path = dir.path().join("question.txt");
        let notice_path = dir.path().join("skipped.tmpl");
        let output_path = dir.path().join("answer.txt");
        std::fs::write(&prompt_path, "Return an answer object").unwrap();
        std::fs::write(&notice_path, "repair skipped").unwrap();

        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: load_prompt
    model: mock:good
  - id: validate
    op: validate_json
    from: draft
    schema: '{{"type":"object","required":["answer"]}}'
  - id: repair
    op: repair
    from: validate
    when: invalid
    model: mock:repair
  - id: skipped_notice
    op: template
    from: repair
    when: skipped
    path: {}
outputs:
  final:
    from: skipped_notice
    path: {}
"#,
            prompt_path.display(),
            notice_path.display(),
            output_path.display()
        ))
        .unwrap();

        let engine = Engine::new()
            .with_backend(
                "mock:good",
                Arc::new(MockBackend::new("mock:good", r#"{"answer":"ok"}"#)),
            )
            .with_backend(
                "mock:repair",
                Arc::new(MockBackend::new("mock:repair", r#"{"answer":"repaired"}"#)),
            );

        let report = engine.run_manifest(manifest, dir.path()).await.unwrap();

        assert_eq!(report.final_status, RunStatus::Succeeded);
        assert_eq!(
            std::fs::read_to_string(output_path).unwrap(),
            "repair skipped"
        );
    }

    #[tokio::test]
    async fn writes_trace_events_for_pipeline_run() {
        let dir = tempfile::tempdir().unwrap();
        let prompt_path = dir.path().join("question.txt");
        let output_path = dir.path().join("answer.txt");
        let trace_path = dir.path().join("trace.jsonl");
        std::fs::write(&prompt_path, "hello").unwrap();

        let manifest = crate::manifest::Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
outputs:
  final:
    from: load_prompt
    path: {}
"#,
            prompt_path.display(),
            output_path.display()
        ))
        .unwrap();

        let engine = Engine::new();
        let options = RunOptions {
            run_id: "test-run".to_string(),
            trace_path: Some(trace_path.clone()),
            event_path: None,
            scheduler: SchedulerMode::Sequential,
            plugin_dirs: Vec::new(),
            stream_stage: None,
            stream_path: None,
            max_concurrency: None,
            default_timeout_ms: None,
            default_retry: RetryPolicy::default(),
            checkpoint_path: None,
            resume_path: None,
            replay_trace_path: None,
        };

        engine
            .run_manifest_with_options(manifest, dir.path(), options)
            .await
            .unwrap();

        let trace = std::fs::read_to_string(trace_path).unwrap();
        assert!(trace.contains(r#""event":"run_started""#));
        assert!(trace.contains(r#""event":"stage_started""#));
        assert!(trace.contains(r#""event":"stage_finished""#));
        assert!(trace.contains(r#""event":"run_finished""#));
    }

    #[tokio::test]
    async fn trace_events_include_timestamps_and_stage_durations() {
        let dir = tempfile::tempdir().unwrap();
        let prompt_path = dir.path().join("question.txt");
        let output_path = dir.path().join("answer.txt");
        let trace_path = dir.path().join("trace.jsonl");
        std::fs::write(&prompt_path, "hello").unwrap();

        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
outputs:
  final:
    from: load_prompt
    path: {}
"#,
            prompt_path.display(),
            output_path.display()
        ))
        .unwrap();
        let options = RunOptions {
            run_id: "trace-test".to_string(),
            trace_path: Some(trace_path.clone()),
            event_path: None,
            scheduler: SchedulerMode::Sequential,
            plugin_dirs: Vec::new(),
            stream_stage: None,
            stream_path: None,
            max_concurrency: None,
            default_timeout_ms: None,
            default_retry: RetryPolicy::default(),
            checkpoint_path: None,
            resume_path: None,
            replay_trace_path: None,
        };

        Engine::new()
            .run_manifest_with_options(manifest, dir.path(), options)
            .await
            .unwrap();

        let trace = std::fs::read_to_string(trace_path).unwrap();
        let events = parse_trace_events(&trace);

        assert!(events.iter().all(|event| event["timestamp_ms"].is_u64()));
        let stage_finished = events
            .iter()
            .find(|event| event["event"] == "stage_finished")
            .expect("stage_finished event should exist");
        assert!(stage_finished["duration_ms"].is_u64());
    }

    #[tokio::test]
    async fn trace_events_include_run_failed_without_sensitive_payloads() {
        let dir = tempfile::tempdir().unwrap();
        let prompt_path = dir.path().join("secret-question.txt");
        let trace_path = dir.path().join("trace.jsonl");
        std::fs::write(&prompt_path, "super secret prompt body").unwrap();

        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: load_prompt
    model: missing:model
outputs:
  final:
    from: draft
    path: answer.txt
"#,
            prompt_path.display()
        ))
        .unwrap();
        let options = RunOptions {
            run_id: "failed-run".to_string(),
            trace_path: Some(trace_path.clone()),
            event_path: None,
            scheduler: SchedulerMode::Sequential,
            plugin_dirs: Vec::new(),
            stream_stage: None,
            stream_path: None,
            max_concurrency: None,
            default_timeout_ms: None,
            default_retry: RetryPolicy::default(),
            checkpoint_path: None,
            resume_path: None,
            replay_trace_path: None,
        };

        let error = Engine::new()
            .run_manifest_with_options(manifest, dir.path(), options)
            .await
            .expect_err("run should fail when model backend is missing");
        assert!(matches!(error, LlmffError::Backend(_)));

        let trace = std::fs::read_to_string(trace_path).unwrap();
        let events = parse_trace_events(&trace);
        let failed = events
            .iter()
            .find(|event| event["event"] == "run_failed")
            .expect("run_failed event should exist");

        assert_eq!(failed["run_id"], "failed-run");
        assert_eq!(failed["status"], "failed");
        assert_eq!(failed["failure_kind"], "backend");
        assert_eq!(failed["failure_message"], "backend request failed");
        assert!(failed["stage_id"].is_null());
        assert!(failed["timestamp_ms"].is_u64());
        assert!(!trace.contains("super secret prompt body"));
        assert!(!trace.contains("missing:model"));
    }

    #[tokio::test]
    async fn trace_events_include_skipped_when_stage_status() {
        let dir = tempfile::tempdir().unwrap();
        let prompt_path = dir.path().join("question.txt");
        let output_path = dir.path().join("answer.json");
        let trace_path = dir.path().join("trace.jsonl");
        std::fs::write(&prompt_path, "Return an answer object").unwrap();

        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: load_prompt
    model: mock:good
  - id: validate
    op: validate_json
    from: draft
    schema: '{{"type":"object","required":["answer"]}}'
  - id: repair
    op: repair
    from: validate
    when: invalid
    model: mock:repair
  - id: choose_final
    op: route
    from: validate
    on_success: validate
    on_invalid: repair
outputs:
  final:
    from: choose_final
    path: {}
"#,
            prompt_path.display(),
            output_path.display()
        ))
        .unwrap();
        let engine = Engine::new()
            .with_backend(
                "mock:good",
                Arc::new(MockBackend::new("mock:good", r#"{"answer":"ok"}"#)),
            )
            .with_backend(
                "mock:repair",
                Arc::new(MockBackend::new("mock:repair", r#"{"answer":"repaired"}"#)),
            );

        engine
            .run_manifest_with_options(
                manifest,
                dir.path(),
                RunOptions {
                    run_id: "trace-test".to_string(),
                    trace_path: Some(trace_path.clone()),
                    event_path: None,
                    scheduler: SchedulerMode::Sequential,
                    plugin_dirs: Vec::new(),
                    stream_stage: None,
                    stream_path: None,
                    max_concurrency: None,
                    default_timeout_ms: None,
                    default_retry: RetryPolicy::default(),
                    checkpoint_path: None,
                    resume_path: None,
                    replay_trace_path: None,
                },
            )
            .await
            .unwrap();

        let trace = std::fs::read_to_string(trace_path).unwrap();
        let events = parse_trace_events(&trace);
        let repair_finished = trace_stage_finished(&events, "repair");

        assert_eq!(repair_finished["status"], "skipped");
    }

    #[tokio::test]
    async fn trace_events_include_infer_model_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let prompt_path = dir.path().join("question.txt");
        let output_path = dir.path().join("answer.txt");
        let trace_path = dir.path().join("trace.jsonl");
        std::fs::write(&prompt_path, "hello").unwrap();

        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: load_prompt
    model: openai:gpt-test
outputs:
  final:
    from: draft
    path: {}
"#,
            prompt_path.display(),
            output_path.display()
        ))
        .unwrap();
        let engine =
            Engine::new().with_backend("openai", Arc::new(MockBackend::new("gpt-test", "ok")));

        engine
            .run_manifest_with_options(
                manifest,
                dir.path(),
                RunOptions {
                    run_id: "trace-test".to_string(),
                    trace_path: Some(trace_path.clone()),
                    event_path: None,
                    scheduler: SchedulerMode::Sequential,
                    plugin_dirs: Vec::new(),
                    stream_stage: None,
                    stream_path: None,
                    max_concurrency: None,
                    default_timeout_ms: None,
                    default_retry: RetryPolicy::default(),
                    checkpoint_path: None,
                    resume_path: None,
                    replay_trace_path: None,
                },
            )
            .await
            .unwrap();

        let trace = std::fs::read_to_string(trace_path).unwrap();
        let events = parse_trace_events(&trace);
        let draft_finished = trace_stage_finished(&events, "draft");

        assert_eq!(draft_finished["model"], "openai:gpt-test");
        assert_eq!(draft_finished["backend"], "openai");
        assert_eq!(draft_finished["provider_model"], "gpt-test");
    }

    #[tokio::test]
    async fn trace_events_include_model_usage_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let prompt_path = dir.path().join("question.txt");
        let output_path = dir.path().join("answer.txt");
        let trace_path = dir.path().join("trace.jsonl");
        std::fs::write(&prompt_path, "hello").unwrap();

        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: load_prompt
    model: usage:test-model
outputs:
  final:
    from: draft
    path: {}
"#,
            prompt_path.display(),
            output_path.display()
        ))
        .unwrap();
        let engine = Engine::new().with_backend(
            "usage",
            Arc::new(UsageBackend {
                model: "test-model".to_string(),
                text: "ok".to_string(),
                usage: UsageMetadata {
                    prompt_tokens: Some(12),
                    completion_tokens: Some(8),
                    total_tokens: Some(20),
                },
            }),
        );

        engine
            .run_manifest_with_options(
                manifest,
                dir.path(),
                RunOptions {
                    run_id: "trace-test".to_string(),
                    trace_path: Some(trace_path.clone()),
                    event_path: None,
                    scheduler: SchedulerMode::Sequential,
                    plugin_dirs: Vec::new(),
                    stream_stage: None,
                    stream_path: None,
                    max_concurrency: None,
                    default_timeout_ms: None,
                    default_retry: RetryPolicy::default(),
                    checkpoint_path: None,
                    resume_path: None,
                    replay_trace_path: None,
                },
            )
            .await
            .unwrap();

        let trace = std::fs::read_to_string(trace_path).unwrap();
        let events = parse_trace_events(&trace);
        let draft_finished = trace_stage_finished(&events, "draft");

        assert_eq!(draft_finished["prompt_tokens"], 12);
        assert_eq!(draft_finished["completion_tokens"], 8);
        assert_eq!(draft_finished["total_tokens"], 20);
    }

    #[tokio::test]
    async fn trace_events_include_validation_errors() {
        let dir = tempfile::tempdir().unwrap();
        let prompt_path = dir.path().join("question.txt");
        let output_path = dir.path().join("answer.txt");
        let trace_path = dir.path().join("trace.jsonl");
        std::fs::write(&prompt_path, r#"{"wrong":true}"#).unwrap();

        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: validate
    op: validate_json
    from: load_prompt
    schema: '{{"type":"object","required":["answer"]}}'
  - id: save_invalid
    op: route
    from: validate
    on_invalid: validate
outputs:
  final:
    from: save_invalid
    path: {}
"#,
            prompt_path.display(),
            output_path.display()
        ))
        .unwrap();

        let error = Engine::new()
            .run_manifest_with_options(
                manifest,
                dir.path(),
                RunOptions {
                    run_id: "trace-test".to_string(),
                    trace_path: Some(trace_path.clone()),
                    event_path: None,
                    scheduler: SchedulerMode::Sequential,
                    plugin_dirs: Vec::new(),
                    stream_stage: None,
                    stream_path: None,
                    max_concurrency: None,
                    default_timeout_ms: None,
                    default_retry: RetryPolicy::default(),
                    checkpoint_path: None,
                    resume_path: None,
                    replay_trace_path: None,
                },
            )
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("output references invalid stage"));

        let trace = std::fs::read_to_string(trace_path).unwrap();
        let events = parse_trace_events(&trace);
        let validate_finished = trace_stage_finished(&events, "validate");

        assert!(
            validate_finished["validation_errors"].as_array().unwrap()[0]
                .as_str()
                .unwrap()
                .contains("answer")
        );
    }

    #[tokio::test]
    async fn trace_events_include_tool_and_write_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let prompt_path = dir.path().join("question.txt");
        let written_path = dir.path().join("written.txt");
        let trace_path = dir.path().join("trace.jsonl");
        std::fs::write(&prompt_path, "hello").unwrap();

        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: call_tool
    op: tool
    from: load_prompt
    command: ["/bin/cat"]
  - id: write_tool
    op: write
    from: call_tool
    path: {}
"#,
            prompt_path.display(),
            written_path.display()
        ))
        .unwrap();

        Engine::new()
            .run_manifest_with_options(
                manifest,
                dir.path(),
                RunOptions {
                    run_id: "trace-test".to_string(),
                    trace_path: Some(trace_path.clone()),
                    event_path: None,
                    scheduler: SchedulerMode::Sequential,
                    plugin_dirs: Vec::new(),
                    stream_stage: None,
                    stream_path: None,
                    max_concurrency: None,
                    default_timeout_ms: None,
                    default_retry: RetryPolicy::default(),
                    checkpoint_path: None,
                    resume_path: None,
                    replay_trace_path: None,
                },
            )
            .await
            .unwrap();

        let trace = std::fs::read_to_string(trace_path).unwrap();
        let events = parse_trace_events(&trace);
        let tool_finished = trace_stage_finished(&events, "call_tool");
        let write_finished = trace_stage_finished(&events, "write_tool");

        assert_eq!(tool_finished["tool_kind"], "command");
        assert_eq!(tool_finished["tool_target"], "/bin/cat");
        assert_eq!(
            write_finished["output_path"].as_str(),
            Some(written_path.to_string_lossy().as_ref())
        );
    }

    #[tokio::test]
    async fn trace_events_include_cache_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let prompt_path = dir.path().join("prompt.txt");
        let first_trace_path = dir.path().join("trace-first.jsonl");
        let second_trace_path = dir.path().join("trace-second.jsonl");
        std::fs::write(&prompt_path, "first").unwrap();

        let engine = Engine::new();
        let manifest = cache_manifest("answer-v1");
        engine
            .run_manifest_with_options(
                manifest.clone(),
                dir.path(),
                RunOptions {
                    run_id: "cache-miss".to_string(),
                    trace_path: Some(first_trace_path.clone()),
                    event_path: None,
                    scheduler: SchedulerMode::Sequential,
                    plugin_dirs: Vec::new(),
                    stream_stage: None,
                    stream_path: None,
                    max_concurrency: None,
                    default_timeout_ms: None,
                    default_retry: RetryPolicy::default(),
                    checkpoint_path: None,
                    resume_path: None,
                    replay_trace_path: None,
                },
            )
            .await
            .unwrap();

        std::fs::write(&prompt_path, "secret-prompt-beta").unwrap();
        engine
            .run_manifest_with_options(
                manifest,
                dir.path(),
                RunOptions {
                    run_id: "cache-hit".to_string(),
                    trace_path: Some(second_trace_path.clone()),
                    event_path: None,
                    scheduler: SchedulerMode::Sequential,
                    plugin_dirs: Vec::new(),
                    stream_stage: None,
                    stream_path: None,
                    max_concurrency: None,
                    default_timeout_ms: None,
                    default_retry: RetryPolicy::default(),
                    checkpoint_path: None,
                    resume_path: None,
                    replay_trace_path: None,
                },
            )
            .await
            .unwrap();

        let first_trace = std::fs::read_to_string(first_trace_path).unwrap();
        let second_trace = std::fs::read_to_string(second_trace_path).unwrap();
        let first_events = parse_trace_events(&first_trace);
        let second_events = parse_trace_events(&second_trace);
        let first_cached = trace_stage_finished(&first_events, "cached");
        let second_cached = trace_stage_finished(&second_events, "cached");

        assert_eq!(first_cached["cache_hit"], false);
        assert_eq!(second_cached["cache_hit"], true);
        assert_eq!(first_cached["cache_path"], ".llmff/cache");
        assert_eq!(second_cached["cache_path"], ".llmff/cache");
        assert!(!first_trace.contains("secret-prompt-alpha"));
        assert!(!second_trace.contains("secret-prompt-alpha"));
    }

    #[tokio::test]
    async fn alias_backend_receives_provider_model_id() {
        let dir = tempfile::tempdir().unwrap();
        let prompt_path = dir.path().join("question.txt");
        let output_path = dir.path().join("answer.txt");
        std::fs::write(&prompt_path, "Say hello").unwrap();

        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: load_prompt
    model: openai:gpt-test
outputs:
  final:
    from: draft
    path: {}
"#,
            prompt_path.display(),
            output_path.display()
        ))
        .unwrap();

        let engine = Engine::new().with_backend(
            "openai",
            Arc::new(MockBackend::new("gpt-test", "hello from alias")),
        );

        engine.run_manifest(manifest, dir.path()).await.unwrap();

        assert_eq!(
            std::fs::read_to_string(output_path).unwrap(),
            "hello from alias"
        );
    }

    fn parse_trace_events(trace: &str) -> Vec<serde_json::Value> {
        trace
            .lines()
            .map(|line| serde_json::from_str(line).expect("trace line should be JSON"))
            .collect()
    }

    fn trace_stage_finished<'a>(
        events: &'a [serde_json::Value],
        stage_id: &str,
    ) -> &'a serde_json::Value {
        events
            .iter()
            .find(|event| event["event"] == "stage_finished" && event["stage_id"] == stage_id)
            .expect("stage_finished event should exist")
    }

    #[tokio::test]
    async fn runs_template_stage_before_infer() {
        let dir = tempfile::tempdir().unwrap();
        let prompt_path = dir.path().join("question.txt");
        let template_path = dir.path().join("prompt.tmpl");
        let output_path = dir.path().join("answer.txt");
        std::fs::write(&prompt_path, "Return JSON.").unwrap();
        std::fs::write(&template_path, "Request: {{input}}").unwrap();

        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: render_prompt
    op: template
    from: load_prompt
    path: {}
  - id: draft
    op: infer
    from: render_prompt
    model: mock:json
outputs:
  final:
    from: draft
    path: {}
"#,
            prompt_path.display(),
            template_path.display(),
            output_path.display()
        ))
        .unwrap();

        let engine = Engine::new().with_backend(
            "mock:json",
            Arc::new(MockBackend::new("mock:json", "template worked")),
        );

        engine.run_manifest(manifest, dir.path()).await.unwrap();

        assert_eq!(
            std::fs::read_to_string(output_path).unwrap(),
            "template worked"
        );
    }

    #[tokio::test]
    async fn infer_receives_system_and_user_messages() {
        let dir = tempfile::tempdir().unwrap();
        let prompt_path = dir.path().join("question.txt");
        let policy_path = dir.path().join("policy.md");
        let output_path = dir.path().join("answer.txt");
        std::fs::write(&prompt_path, "Return an answer.").unwrap();
        std::fs::write(&policy_path, "Use terse JSON.").unwrap();

        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: apply_policy
    op: system
    from: load_prompt
    path: {}
  - id: draft
    op: infer
    from: apply_policy
    model: recording:test-model
outputs:
  final:
    from: draft
    path: {}
"#,
            prompt_path.display(),
            policy_path.display(),
            output_path.display()
        ))
        .unwrap();
        let messages = Arc::new(Mutex::new(Vec::new()));
        let seed = Arc::new(Mutex::new(None));
        let response_format = Arc::new(Mutex::new(None));
        let stop = Arc::new(Mutex::new(Vec::new()));
        let engine = Engine::new().with_backend(
            "recording",
            Arc::new(RecordingBackend {
                model: "test-model".to_string(),
                messages: Arc::clone(&messages),
                seed,
                response_format,
                stop,
            }),
        );

        engine.run_manifest(manifest, dir.path()).await.unwrap();

        assert_eq!(
            *messages.lock().unwrap(),
            vec![
                Message {
                    role: "system".to_string(),
                    content: "Use terse JSON.".to_string(),
                },
                Message {
                    role: "user".to_string(),
                    content: "Return an answer.".to_string(),
                },
            ]
        );
    }

    #[tokio::test]
    async fn infer_forwards_seed_to_backend() {
        let dir = tempfile::tempdir().unwrap();
        let prompt_path = dir.path().join("question.txt");
        std::fs::write(&prompt_path, "Return an answer.").unwrap();

        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: load_prompt
    model: recording:test-model
    seed: 12345
"#,
            prompt_path.display()
        ))
        .unwrap();
        let messages = Arc::new(Mutex::new(Vec::new()));
        let seed = Arc::new(Mutex::new(None));
        let response_format = Arc::new(Mutex::new(None));
        let stop = Arc::new(Mutex::new(Vec::new()));
        let engine = Engine::new().with_backend(
            "recording",
            Arc::new(RecordingBackend {
                model: "test-model".to_string(),
                messages,
                seed: Arc::clone(&seed),
                response_format,
                stop,
            }),
        );

        engine.run_manifest(manifest, dir.path()).await.unwrap();

        assert_eq!(*seed.lock().unwrap(), Some(12345));
    }

    #[tokio::test]
    async fn infer_forwards_response_format_to_backend() {
        let dir = tempfile::tempdir().unwrap();
        let prompt_path = dir.path().join("question.txt");
        std::fs::write(&prompt_path, "Return an answer.").unwrap();

        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: load_prompt
    model: recording:test-model
    response_format: json
"#,
            prompt_path.display()
        ))
        .unwrap();
        let messages = Arc::new(Mutex::new(Vec::new()));
        let seed = Arc::new(Mutex::new(None));
        let response_format = Arc::new(Mutex::new(None));
        let stop = Arc::new(Mutex::new(Vec::new()));
        let engine = Engine::new().with_backend(
            "recording",
            Arc::new(RecordingBackend {
                model: "test-model".to_string(),
                messages,
                seed,
                response_format: Arc::clone(&response_format),
                stop,
            }),
        );

        engine.run_manifest(manifest, dir.path()).await.unwrap();

        assert_eq!(*response_format.lock().unwrap(), Some("json".to_string()));
    }

    #[tokio::test]
    async fn infer_forwards_stop_sequences_to_backend() {
        let dir = tempfile::tempdir().unwrap();
        let prompt_path = dir.path().join("question.txt");
        std::fs::write(&prompt_path, "Return an answer.").unwrap();

        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: load_prompt
    model: recording:test-model
    stop:
      - "\nEND"
      - "</answer>"
"#,
            prompt_path.display()
        ))
        .unwrap();
        let messages = Arc::new(Mutex::new(Vec::new()));
        let seed = Arc::new(Mutex::new(None));
        let response_format = Arc::new(Mutex::new(None));
        let stop = Arc::new(Mutex::new(Vec::new()));
        let engine = Engine::new().with_backend(
            "recording",
            Arc::new(RecordingBackend {
                model: "test-model".to_string(),
                messages,
                seed,
                response_format,
                stop: Arc::clone(&stop),
            }),
        );

        engine.run_manifest(manifest, dir.path()).await.unwrap();

        assert_eq!(*stop.lock().unwrap(), vec!["\nEND", "</answer>"]);
    }

    #[tokio::test]
    async fn default_scheduler_runs_ready_model_stages_sequentially() {
        let (manifest, dir, active, max_active) = parallel_scheduler_fixture();
        let engine = Engine::new().with_backend(
            "delay",
            Arc::new(DelayedBackend {
                active,
                max_active: Arc::clone(&max_active),
            }),
        );

        engine.run_manifest(manifest, dir.path()).await.unwrap();

        assert_eq!(max_active.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn parallel_scheduler_runs_ready_model_stages_concurrently() {
        let (manifest, dir, active, max_active) = parallel_scheduler_fixture();
        let engine = Engine::new().with_backend(
            "delay",
            Arc::new(DelayedBackend {
                active,
                max_active: Arc::clone(&max_active),
            }),
        );

        engine
            .run_manifest_with_options(
                manifest,
                dir.path(),
                RunOptions {
                    run_id: "parallel-test".to_string(),
                    trace_path: None,
                    event_path: None,
                    scheduler: SchedulerMode::Parallel,
                    plugin_dirs: Vec::new(),
                    stream_stage: None,
                    stream_path: None,
                    max_concurrency: None,
                    default_timeout_ms: None,
                    default_retry: RetryPolicy::default(),
                    checkpoint_path: None,
                    resume_path: None,
                    replay_trace_path: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(max_active.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn route_stage_selects_success_target() {
        let dir = tempfile::tempdir().unwrap();
        let prompt_path = dir.path().join("question.txt");
        let output_path = dir.path().join("answer.txt");
        std::fs::write(&prompt_path, r#"{"answer":"ok"}"#).unwrap();

        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: validate
    op: validate_json
    from: load_prompt
    schema: '{{"type":"object","required":["answer"]}}'
  - id: choose
    op: route
    from: validate
    on_success: validate
outputs:
  final:
    from: choose
    path: {}
"#,
            prompt_path.display(),
            output_path.display()
        ))
        .unwrap();

        Engine::new()
            .run_manifest(manifest, dir.path())
            .await
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(output_path).unwrap(),
            r#"{"answer":"ok"}"#
        );
    }

    fn parallel_scheduler_fixture() -> (
        Manifest,
        tempfile::TempDir,
        Arc<AtomicUsize>,
        Arc<AtomicUsize>,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let prompt_path = dir.path().join("question.txt");
        let output_path = dir.path().join("answer.txt");
        std::fs::write(&prompt_path, "Say hello").unwrap();

        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: draft_a
    op: infer
    from: load_prompt
    model: delay:a
  - id: draft_b
    op: infer
    from: load_prompt
    model: delay:b
outputs:
  final:
    from: draft_a
    path: {}
"#,
            prompt_path.display(),
            output_path.display()
        ))
        .unwrap();

        (
            manifest,
            dir,
            Arc::new(AtomicUsize::new(0)),
            Arc::new(AtomicUsize::new(0)),
        )
    }

    #[tokio::test]
    async fn route_stage_selects_invalid_target() {
        let dir = tempfile::tempdir().unwrap();
        let prompt_path = dir.path().join("question.txt");
        let output_path = dir.path().join("answer.txt");
        std::fs::write(&prompt_path, r#"{"wrong":true}"#).unwrap();

        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: validate
    op: validate_json
    from: load_prompt
    schema: '{{"type":"object","required":["answer"]}}'
  - id: repair
    op: repair
    from: validate
    model: mock:json
  - id: choose
    op: route
    from: validate
    on_invalid: repair
outputs:
  final:
    from: choose
    path: {}
"#,
            prompt_path.display(),
            output_path.display()
        ))
        .unwrap();
        let engine = Engine::new().with_backend(
            "mock:json",
            Arc::new(MockBackend::new("mock:json", r#"{"answer":"fixed"}"#)),
        );

        engine.run_manifest(manifest, dir.path()).await.unwrap();

        assert_eq!(
            std::fs::read_to_string(output_path).unwrap(),
            r#"{"answer":"fixed"}"#
        );
    }

    #[tokio::test]
    async fn route_stage_selects_json_field_case_target() {
        let dir = tempfile::tempdir().unwrap();
        let source_path = dir.path().join("source.json");
        let fast_path = dir.path().join("fast.txt");
        let strong_path = dir.path().join("strong.txt");
        let output_path = dir.path().join("answer.txt");
        std::fs::write(&source_path, r#"{"kind":"hard"}"#).unwrap();
        std::fs::write(&fast_path, "fast").unwrap();
        std::fs::write(&strong_path, "strong").unwrap();

        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  source:
    path: {}
  fast:
    path: {}
  strong:
    path: {}
graph:
  - id: load_source
    op: load
    input: source
  - id: parse_source
    op: validate_json
    from: load_source
    schema: '{{"type":"object","required":["kind"]}}'
  - id: fast_answer
    op: load
    input: fast
  - id: strong_answer
    op: load
    input: strong
  - id: choose
    op: route
    from: parse_source
    field: kind
    cases:
      hard: strong_answer
      simple: fast_answer
outputs:
  final:
    from: choose
    path: {}
"#,
            source_path.display(),
            fast_path.display(),
            strong_path.display(),
            output_path.display()
        ))
        .unwrap();

        Engine::new()
            .run_manifest(manifest, dir.path())
            .await
            .unwrap();

        assert_eq!(std::fs::read_to_string(output_path).unwrap(), "strong");
    }

    #[tokio::test]
    async fn route_stage_uses_default_for_unmatched_json_field() {
        let dir = tempfile::tempdir().unwrap();
        let source_path = dir.path().join("source.json");
        let fallback_path = dir.path().join("fallback.txt");
        let output_path = dir.path().join("answer.txt");
        std::fs::write(&source_path, r#"{"kind":"unknown"}"#).unwrap();
        std::fs::write(&fallback_path, "fallback").unwrap();

        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  source:
    path: {}
  fallback:
    path: {}
graph:
  - id: load_source
    op: load
    input: source
  - id: parse_source
    op: validate_json
    from: load_source
    schema: '{{"type":"object","required":["kind"]}}'
  - id: fallback_answer
    op: load
    input: fallback
  - id: choose
    op: route
    from: parse_source
    field: kind
    cases:
      hard: fallback_answer
    default: fallback_answer
outputs:
  final:
    from: choose
    path: {}
"#,
            source_path.display(),
            fallback_path.display(),
            output_path.display()
        ))
        .unwrap();

        Engine::new()
            .run_manifest(manifest, dir.path())
            .await
            .unwrap();

        assert_eq!(std::fs::read_to_string(output_path).unwrap(), "fallback");
    }

    #[tokio::test]
    async fn tool_stage_command_receives_parent_on_stdin_and_returns_stdout() {
        let dir = tempfile::tempdir().unwrap();
        let input_path = dir.path().join("input.txt");
        let output_path = dir.path().join("answer.txt");
        std::fs::write(&input_path, "hello tool").unwrap();

        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  source:
    path: {}
graph:
  - id: load_source
    op: load
    input: source
  - id: call_tool
    op: tool
    from: load_source
    command: ["/bin/cat"]
outputs:
  final:
    from: call_tool
    path: {}
"#,
            input_path.display(),
            output_path.display()
        ))
        .unwrap();

        Engine::new()
            .run_manifest(manifest, dir.path())
            .await
            .unwrap();

        assert_eq!(std::fs::read_to_string(output_path).unwrap(), "hello tool");
    }

    #[tokio::test]
    async fn tool_stage_command_reports_nonzero_exit() {
        let dir = tempfile::tempdir().unwrap();
        let input_path = dir.path().join("input.txt");
        let output_path = dir.path().join("answer.txt");
        let tool_path = dir.path().join("fail-tool");
        std::fs::write(&input_path, "hello tool").unwrap();
        std::fs::write(
            &tool_path,
            r#"#!/bin/sh
cat >/dev/null
exit 7
"#,
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&tool_path).unwrap().permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            permissions.set_mode(0o755);
        }
        std::fs::set_permissions(&tool_path, permissions).unwrap();

        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  source:
    path: {}
graph:
  - id: load_source
    op: load
    input: source
  - id: call_tool
    op: tool
    from: load_source
    command: [{}]
outputs:
  final:
    from: call_tool
    path: {}
"#,
            input_path.display(),
            tool_path.display(),
            output_path.display()
        ))
        .unwrap();

        let error = Engine::new()
            .run_manifest(manifest, dir.path())
            .await
            .unwrap_err()
            .to_string();

        assert!(error.contains("tool command exited with status"));
    }

    #[tokio::test]
    async fn tool_stage_uses_plugin_tool_transport() {
        let dir = tempfile::tempdir().unwrap();
        let plugin_dir = dir.path().join("plugins");
        let plugin = plugin_dir.join("cat-plugin");
        std::fs::create_dir_all(&plugin).unwrap();
        std::fs::write(
            plugin.join("llmff-plugin.yaml"),
            r#"
name: cat-plugin
version: 0.1.0
capabilities:
  - kind: tool-transport
    name: stdio-cat
    entrypoint: /bin/cat
"#,
        )
        .unwrap();

        let input_path = dir.path().join("input.txt");
        let output_path = dir.path().join("answer.txt");
        std::fs::write(&input_path, "hello plugin").unwrap();

        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  source:
    path: {}
graph:
  - id: load_source
    op: load
    input: source
  - id: call_tool
    op: tool
    from: load_source
    transport: stdio-cat
outputs:
  final:
    from: call_tool
    path: {}
"#,
            input_path.display(),
            output_path.display()
        ))
        .unwrap();

        Engine::new()
            .run_manifest_with_options(
                manifest,
                dir.path(),
                RunOptions {
                    run_id: "plugin-test".to_string(),
                    trace_path: None,
                    event_path: None,
                    scheduler: SchedulerMode::Sequential,
                    plugin_dirs: vec![plugin_dir],
                    stream_stage: None,
                    stream_path: None,
                    max_concurrency: None,
                    default_timeout_ms: None,
                    default_retry: RetryPolicy::default(),
                    checkpoint_path: None,
                    resume_path: None,
                    replay_trace_path: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(output_path).unwrap(),
            "hello plugin"
        );
    }

    #[tokio::test]
    async fn tool_stage_posts_parent_body_to_http_endpoint() {
        let dir = tempfile::tempdir().unwrap();
        let input_path = dir.path().join("input.txt");
        let output_path = dir.path().join("answer.txt");
        std::fs::write(&input_path, "ping").unwrap();
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/process"))
            .and(header("content-type", "text/plain"))
            .and(body_string("ping"))
            .respond_with(ResponseTemplate::new(200).set_body_string("pong"))
            .mount(&server)
            .await;

        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  source:
    path: {}
graph:
  - id: load_source
    op: load
    input: source
  - id: call_tool
    op: tool
    from: load_source
    method: POST
    url: {}/process
    headers:
      content-type: text/plain
outputs:
  final:
    from: call_tool
    path: {}
"#,
            input_path.display(),
            server.uri(),
            output_path.display()
        ))
        .unwrap();

        Engine::new()
            .run_manifest(manifest, dir.path())
            .await
            .unwrap();

        assert_eq!(std::fs::read_to_string(output_path).unwrap(), "pong");
    }

    #[tokio::test]
    async fn write_stage_writes_and_forwards_parent_value() {
        let dir = tempfile::tempdir().unwrap();
        let input_path = dir.path().join("input.json");
        let written_path = dir.path().join("written.json");
        let output_path = dir.path().join("answer.json");
        std::fs::write(&input_path, r#"{"answer":"ok"}"#).unwrap();

        let manifest = Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  source:
    path: {}
graph:
  - id: load_source
    op: load
    input: source
  - id: parse_source
    op: validate_json
    from: load_source
    schema: '{{"type":"object","required":["answer"]}}'
  - id: save_source
    op: write
    from: parse_source
    path: {}
outputs:
  final:
    from: save_source
    path: {}
"#,
            input_path.display(),
            written_path.display(),
            output_path.display()
        ))
        .unwrap();

        Engine::new()
            .run_manifest(manifest, dir.path())
            .await
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(&written_path).unwrap(),
            r#"{"answer":"ok"}"#
        );
        assert_eq!(
            std::fs::read_to_string(&output_path).unwrap(),
            r#"{"answer":"ok"}"#
        );
    }

    fn cache_manifest(key: &str) -> Manifest {
        Manifest::from_yaml_str(&format!(
            r#"
version: 1
inputs:
  prompt:
    path: prompt.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: cached
    op: cache
    from: load_prompt
    path: .llmff/cache
    key: {key}
outputs:
  final:
    from: cached
    path: answer.txt
"#,
        ))
        .expect("manifest should parse")
    }
}
