use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::backend::{Backend, InferRequest, UsageMetadata};
use crate::error::LlmffError;
use crate::graph::{stage_dependencies, Graph};
use crate::manifest::{Manifest, StageSpec};
use crate::stage::execute_deterministic_stage;
use crate::trace::{TraceEvent, TraceWriter};
use crate::value::{Message, StageStatus, Value};

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

struct StageOutcome {
    status: StageStatus,
    usage: Option<UsageMetadata>,
}

impl StageOutcome {
    fn without_usage(status: StageStatus) -> Self {
        Self {
            status,
            usage: None,
        }
    }

    fn with_usage(status: StageStatus, usage: Option<UsageMetadata>) -> Self {
        Self { status, usage }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOptions {
    pub run_id: String,
    pub trace_path: Option<PathBuf>,
    pub scheduler: SchedulerMode,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            run_id: "local-run".to_string(),
            trace_path: None,
            scheduler: SchedulerMode::Sequential,
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
        validate_input_formats(&manifest)?;
        let graph = Graph::from_manifest(manifest.clone())?;
        for stage in graph.stages() {
            self.validate_stage(stage)?;
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
        let graph = self.validate_manifest(manifest.clone())?;
        let mut statuses = BTreeMap::new();
        let mut trace = match options.trace_path.as_ref() {
            Some(path) => Some(TraceWriter::create(path)?),
            None => None,
        };

        write_trace(
            &mut trace,
            TraceEvent {
                run_id: options.run_id.clone(),
                event: "run_started".to_string(),
                stage_id: None,
                op: None,
                status: None,
                timestamp_ms: timestamp_ms(),
                duration_ms: None,
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
            },
        )?;

        match options.scheduler {
            SchedulerMode::Sequential => {
                self.run_stages_sequentially(
                    &manifest,
                    &graph,
                    cwd,
                    &mut statuses,
                    &mut trace,
                    &options.run_id,
                )
                .await?;
            }
            SchedulerMode::Parallel => {
                self.run_stages_in_parallel(
                    &manifest,
                    &graph,
                    cwd,
                    &mut statuses,
                    &mut trace,
                    &options.run_id,
                )
                .await?;
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
            &mut trace,
            TraceEvent {
                run_id: options.run_id,
                event: "run_finished".to_string(),
                stage_id: None,
                op: None,
                status: Some("succeeded".to_string()),
                timestamp_ms: timestamp_ms(),
                duration_ms: None,
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
            },
        )?;

        Ok(report)
    }

    async fn run_stages_sequentially(
        &self,
        manifest: &Manifest,
        graph: &Graph,
        cwd: &Path,
        statuses: &mut BTreeMap<String, StageStatus>,
        trace: &mut Option<TraceWriter>,
        run_id: &str,
    ) -> Result<(), LlmffError> {
        for stage in graph.stages() {
            let stage_started = self.start_stage_trace(trace, run_id, stage)?;
            let outcome = self.execute_stage(manifest, stage, statuses, cwd).await?;
            self.finish_stage_trace(trace, run_id, stage, stage_started, outcome, statuses)?;
        }

        Ok(())
    }

    async fn run_stages_in_parallel(
        &self,
        manifest: &Manifest,
        graph: &Graph,
        cwd: &Path,
        statuses: &mut BTreeMap<String, StageStatus>,
        trace: &mut Option<TraceWriter>,
        run_id: &str,
    ) -> Result<(), LlmffError> {
        let mut pending = graph.stages().iter().collect::<Vec<_>>();

        while !pending.is_empty() {
            let mut ready = Vec::new();
            let mut waiting = Vec::new();

            for stage in pending {
                if stage_dependencies(stage)
                    .iter()
                    .all(|dependency| statuses.contains_key(dependency))
                {
                    ready.push(stage);
                } else {
                    waiting.push(stage);
                }
            }

            if ready.is_empty() {
                return Err(LlmffError::GraphValidation(
                    "cycle detected in graph".to_string(),
                ));
            }

            let starts = ready
                .iter()
                .map(|stage| self.start_stage_trace(trace, run_id, stage))
                .collect::<Result<Vec<_>, _>>()?;
            let status_snapshot = statuses.clone();
            let outcomes = futures::future::join_all(
                ready
                    .iter()
                    .map(|stage| self.execute_stage(manifest, stage, &status_snapshot, cwd)),
            )
            .await;

            for ((stage, started), outcome) in ready.into_iter().zip(starts).zip(outcomes) {
                self.finish_stage_trace(trace, run_id, stage, started, outcome?, statuses)?;
            }

            pending = waiting;
        }

        Ok(())
    }

    fn start_stage_trace(
        &self,
        trace: &mut Option<TraceWriter>,
        run_id: &str,
        stage: &StageSpec,
    ) -> Result<Instant, LlmffError> {
        let started = Instant::now();
        write_trace(
            trace,
            TraceEvent {
                run_id: run_id.to_string(),
                event: "stage_started".to_string(),
                stage_id: Some(stage.id.clone()),
                op: Some(stage.op.clone()),
                status: None,
                timestamp_ms: timestamp_ms(),
                duration_ms: None,
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
            },
        )?;
        Ok(started)
    }

    fn finish_stage_trace(
        &self,
        trace: &mut Option<TraceWriter>,
        run_id: &str,
        stage: &StageSpec,
        stage_started: Instant,
        outcome: StageOutcome,
        statuses: &mut BTreeMap<String, StageStatus>,
    ) -> Result<(), LlmffError> {
        let status = outcome.status;
        let status_name = status_name(&status).to_string();
        let metadata = self.trace_metadata(stage, &status, outcome.usage.as_ref());
        statuses.insert(stage.id.clone(), status);
        write_trace(
            trace,
            TraceEvent {
                run_id: run_id.to_string(),
                event: "stage_finished".to_string(),
                stage_id: Some(stage.id.clone()),
                op: Some(stage.op.clone()),
                status: Some(status_name),
                timestamp_ms: timestamp_ms(),
                duration_ms: Some(stage_started.elapsed().as_millis()),
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
            },
        )
    }

    fn validate_stage(&self, stage: &StageSpec) -> Result<(), LlmffError> {
        validate_when_condition(stage)?;
        validate_sampling_parameters(stage)?;

        match stage.op.as_str() {
            "load" => {
                require_stage_field(stage, stage.input.as_deref(), "load requires input")?;
                Ok(())
            }
            "infer" => {
                require_stage_field(stage, stage.from.as_deref(), "infer requires from")?;
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
                if stage.documents.is_empty() {
                    return Err(stage_validation_error(stage, "retrieve requires documents"));
                }
                if let Some(0) = stage.top_k {
                    return Err(stage_validation_error(
                        stage,
                        "retrieve top_k must be greater than 0",
                    ));
                }
                Ok(())
            }
            "repair" => {
                require_stage_field(stage, stage.from.as_deref(), "repair requires from")?;
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

    async fn execute_stage(
        &self,
        manifest: &Manifest,
        stage: &StageSpec,
        statuses: &BTreeMap<String, StageStatus>,
        cwd: &Path,
    ) -> Result<StageOutcome, LlmffError> {
        if !should_execute_stage(stage, statuses)? {
            return Ok(StageOutcome::without_usage(StageStatus::Skipped));
        }

        match stage.op.as_str() {
            "load" => self
                .execute_load(manifest, stage, cwd)
                .map(StageOutcome::without_usage),
            "infer" => self.execute_infer(stage, statuses).await,
            "validate_json" | "system" | "template" | "retrieve" => {
                let input = stage
                    .from
                    .as_ref()
                    .and_then(|parent| statuses.get(parent))
                    .and_then(success_value);
                execute_deterministic_stage(stage, input, cwd).map(StageOutcome::without_usage)
            }
            "repair" => self.execute_repair(stage, statuses).await,
            "route" => self
                .execute_route(stage, statuses)
                .map(StageOutcome::without_usage),
            "tool" => self
                .execute_tool(stage, statuses, cwd)
                .await
                .map(StageOutcome::without_usage),
            "write" => self
                .execute_write(stage, statuses, cwd)
                .map(StageOutcome::without_usage),
            other => Err(LlmffError::UnknownStage(other.to_string())),
        }
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
    ) -> Result<StageOutcome, LlmffError> {
        let messages = parent_messages(stage, statuses)?;
        let model = required_model(stage)?;
        let resolved = self.backend_for_model(model)?;
        let response = resolved
            .infer(InferRequest {
                model: resolved.provider_model.to_string(),
                messages,
                temperature: stage.temperature,
                top_p: stage.top_p,
                max_tokens: stage.max_tokens,
            })
            .await?;

        Ok(StageOutcome::with_usage(
            StageStatus::Success(Value::Text(response.text)),
            response.usage,
        ))
    }

    async fn execute_repair(
        &self,
        stage: &StageSpec,
        statuses: &BTreeMap<String, StageStatus>,
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
                let response = resolved
                    .infer(InferRequest {
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
                    })
                    .await?;

                Ok(StageOutcome::with_usage(
                    StageStatus::Success(Value::Text(response.text)),
                    response.usage,
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
    ) -> Result<StageStatus, LlmffError> {
        if let Some(command) = &stage.command {
            return execute_command_tool(stage, statuses, cwd, command);
        }
        if stage.url.is_some() {
            return execute_http_tool(stage, statuses).await;
        }

        Err(LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: "tool requires command or url".to_string(),
        })
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

    Ok(())
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
        "validate_json" | "retrieve" => StageValueKind::Json,
        "write" => stage
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

fn execute_command_tool(
    stage: &StageSpec,
    statuses: &BTreeMap<String, StageStatus>,
    cwd: &Path,
    command: &[String],
) -> Result<StageStatus, LlmffError> {
    let input = parent_text(stage, statuses)?;
    let program = command.first().ok_or_else(|| LlmffError::StageExecution {
        stage_id: stage.id.clone(),
        message: "tool command cannot be empty".to_string(),
    })?;
    let mut child = Command::new(resolve_command_path(cwd, program))
        .args(&command[1..])
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!("failed to start tool command `{program}`: {error}"),
        })?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: "failed to open tool command stdin".to_string(),
        })?;
    stdin
        .write_all(input.as_bytes())
        .map_err(|error| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!("failed to write tool command stdin: {error}"),
        })?;
    drop(stdin);

    let output = child
        .wait_with_output()
        .map_err(|error| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!("failed to wait for tool command `{program}`: {error}"),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!(
                "tool command exited with status {}: {}",
                output.status,
                stderr.trim()
            ),
        });
    }

    let stdout = String::from_utf8(output.stdout).map_err(|error| LlmffError::StageExecution {
        stage_id: stage.id.clone(),
        message: format!("tool command stdout was not valid UTF-8: {error}"),
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

fn write_trace(trace: &mut Option<TraceWriter>, event: TraceEvent) -> Result<(), LlmffError> {
    if let Some(trace) = trace {
        trace.write_event(&event)?;
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
    }

    #[async_trait::async_trait]
    impl Backend for RecordingBackend {
        async fn infer(&self, request: InferRequest) -> Result<InferResponse, LlmffError> {
            assert_eq!(request.model, self.model);
            *self.messages.lock().unwrap() = request.messages;
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
            scheduler: SchedulerMode::Sequential,
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
            scheduler: SchedulerMode::Sequential,
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
                    scheduler: SchedulerMode::Sequential,
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
                    scheduler: SchedulerMode::Sequential,
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
                    scheduler: SchedulerMode::Sequential,
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
                    scheduler: SchedulerMode::Sequential,
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
                    scheduler: SchedulerMode::Sequential,
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
        let engine = Engine::new().with_backend(
            "recording",
            Arc::new(RecordingBackend {
                model: "test-model".to_string(),
                messages: Arc::clone(&messages),
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
                    scheduler: SchedulerMode::Parallel,
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
    command: ["/bin/false"]
outputs:
  final:
    from: call_tool
    path: {}
"#,
            input_path.display(),
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
}
