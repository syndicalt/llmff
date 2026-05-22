use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

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
            },
        )?;

        for stage in graph.stages() {
            write_trace(
                &mut trace,
                TraceEvent {
                    run_id: options.run_id.clone(),
                    event: "stage_started".to_string(),
                    stage_id: Some(stage.id.clone()),
                    op: Some(stage.op.clone()),
                    status: None,
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
            "validate_json" | "system" => {
                let input = stage
                    .from
                    .as_ref()
                    .and_then(|parent| statuses.get(parent))
                    .and_then(success_value);
                execute_deterministic_stage(stage, input, cwd)
            }
            "repair" => self.execute_repair(stage, statuses).await,
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
}
