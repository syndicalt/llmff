use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::backend::{Backend, InferRequest};
use crate::error::LlmffError;
use crate::graph::Graph;
use crate::manifest::{Manifest, StageSpec};
use crate::stage::execute_deterministic_stage;
use crate::trace::{TraceEvent, TraceWriter};
use crate::value::{StageStatus, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunReport {
    pub final_status: RunStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOptions {
    pub run_id: String,
    pub trace_path: Option<PathBuf>,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            run_id: "local-run".to_string(),
            trace_path: None,
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
        let graph = Graph::from_manifest(manifest.clone())?;
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
            },
        )?;

        for stage in graph.stages() {
            let stage_started = Instant::now();
            write_trace(
                &mut trace,
                TraceEvent {
                    run_id: options.run_id.clone(),
                    event: "stage_started".to_string(),
                    stage_id: Some(stage.id.clone()),
                    op: Some(stage.op.clone()),
                    status: None,
                    timestamp_ms: timestamp_ms(),
                    duration_ms: None,
                },
            )?;
            let status = self.execute_stage(&manifest, stage, &statuses, cwd).await?;
            let status_name = status_name(&status).to_string();
            statuses.insert(stage.id.clone(), status);
            write_trace(
                &mut trace,
                TraceEvent {
                    run_id: options.run_id.clone(),
                    event: "stage_finished".to_string(),
                    stage_id: Some(stage.id.clone()),
                    op: Some(stage.op.clone()),
                    status: Some(status_name),
                    timestamp_ms: timestamp_ms(),
                    duration_ms: Some(stage_started.elapsed().as_millis()),
                },
            )?;
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
            },
        )?;

        Ok(report)
    }

    async fn execute_stage(
        &self,
        manifest: &Manifest,
        stage: &StageSpec,
        statuses: &BTreeMap<String, StageStatus>,
        cwd: &Path,
    ) -> Result<StageStatus, LlmffError> {
        match stage.op.as_str() {
            "load" => self.execute_load(manifest, stage, cwd),
            "infer" => self.execute_infer(stage, statuses).await,
            "validate_json" | "system" | "template" => {
                let input = stage
                    .from
                    .as_ref()
                    .and_then(|parent| statuses.get(parent))
                    .and_then(success_value);
                execute_deterministic_stage(stage, input, cwd)
            }
            "repair" => self.execute_repair(stage, statuses).await,
            "route" => self.execute_route(stage, statuses),
            "tool" => self.execute_tool(stage, statuses, cwd).await,
            "write" => self.execute_write(stage, statuses, cwd),
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

        Ok(StageStatus::Success(Value::Text(text)))
    }

    async fn execute_infer(
        &self,
        stage: &StageSpec,
        statuses: &BTreeMap<String, StageStatus>,
    ) -> Result<StageStatus, LlmffError> {
        let prompt = parent_text(stage, statuses)?;
        let model = required_model(stage)?;
        let resolved = self.backend_for_model(model)?;
        let response = resolved
            .infer(InferRequest {
                model: resolved.provider_model.to_string(),
                prompt,
                temperature: stage.temperature,
            })
            .await?;

        Ok(StageStatus::Success(Value::Text(response.text)))
    }

    async fn execute_repair(
        &self,
        stage: &StageSpec,
        statuses: &BTreeMap<String, StageStatus>,
    ) -> Result<StageStatus, LlmffError> {
        let parent = stage
            .from
            .as_ref()
            .and_then(|parent| statuses.get(parent))
            .ok_or_else(|| LlmffError::StageExecution {
                stage_id: stage.id.clone(),
                message: "repair requires parent stage".to_string(),
            })?;

        match parent {
            StageStatus::Success(value) => Ok(StageStatus::Success(value.clone())),
            StageStatus::Skipped => Ok(StageStatus::Skipped),
            StageStatus::Invalid { value, errors } => {
                let model = required_model(stage)?;
                let resolved = self.backend_for_model(model)?;
                let response = resolved
                    .infer(InferRequest {
                        model: resolved.provider_model.to_string(),
                        prompt: format!(
                            "Repair this output so it satisfies validation errors.\nErrors:\n{}\nOutput:\n{}",
                            errors.join("\n"),
                            serialize_value(value)?
                        ),
                        temperature: stage.temperature,
                    })
                    .await?;

                Ok(StageStatus::Success(Value::Text(response.text)))
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
        Value::Json(json) => serde_json::to_string(json).map_err(LlmffError::Json),
    }
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
    use std::sync::Arc;

    use tempfile::tempdir;

    use crate::backend::MockBackend;
    use crate::manifest::Manifest;
    use wiremock::matchers::{body_string, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

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
