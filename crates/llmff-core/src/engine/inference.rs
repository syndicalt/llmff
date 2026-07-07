use std::collections::BTreeMap;
use std::process::Stdio;

use futures::StreamExt;
use serde::Deserialize;
use tokio::io::AsyncWriteExt as _;
use tokio::process::Command;

use crate::backend::{Backend, InferRequest, InferResponse};
use crate::error::LlmffError;
use crate::manifest::StageSpec;
use crate::plugin::PluginSampler;
use crate::stage::specs::{InferSpec, RepairSpec};
use crate::value::{Message, StageStatus, Value};

use super::streaming::StageStreamWriter;
use super::tool_exec::sleep_for_retry;
use super::{retry_policy, serialize_value, Engine, RetryPolicy, StageOutcome};

struct InferAttemptResult {
    response: InferResponse,
    attempts: usize,
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

impl Engine {
    pub(super) async fn execute_infer(
        &self,
        stage: &StageSpec,
        statuses: &BTreeMap<String, StageStatus>,
        plugin_samplers: &BTreeMap<String, PluginSampler>,
        stream_writer: Option<&mut StageStreamWriter>,
        default_retry: RetryPolicy,
    ) -> Result<StageOutcome, LlmffError> {
        let typed = InferSpec::parse(stage).map_err(|message| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message,
        })?;
        let messages =
            with_agent_system_prompt(typed.system.as_deref(), parent_messages(stage, statuses)?);
        let resolved = self.backend_for_model(&typed.model)?;
        let mut request = InferRequest {
            model: resolved.provider_model.to_string(),
            messages,
            temperature: typed.temperature,
            top_p: typed.top_p,
            max_tokens: typed.max_tokens,
            seed: typed.seed,
            response_format: typed.response_format,
            stop: typed.stop,
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

    pub(super) async fn execute_repair(
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
                let typed =
                    RepairSpec::parse(stage).map_err(|message| LlmffError::StageExecution {
                        stage_id: stage.id.clone(),
                        message,
                    })?;
                let resolved = self.backend_for_model(&typed.model)?;
                let mut request = InferRequest {
                    model: resolved.provider_model.to_string(),
                    messages: with_agent_system_prompt(
                        typed.system.as_deref(),
                        vec![Message {
                            role: "user".to_string(),
                            content: format!(
                                "Repair this output so it satisfies validation errors.\nErrors:\n{}\nOutput:\n{}",
                                errors.join("\n"),
                                serialize_value(value)?
                            ),
                        }],
                    ),
                    temperature: typed.temperature,
                    top_p: typed.top_p,
                    max_tokens: typed.max_tokens,
                    seed: typed.seed,
                    response_format: typed.response_format,
                    stop: typed.stop,
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

/// Prepend the stage's resolved `system` persona as a system message, unless
/// the assembled messages already lead with one. The persona usually arrives
/// via an `agent:` reference expanded in `Manifest::resolve_agents`, but an
/// inline `system:` on the stage works the same way.
fn with_agent_system_prompt(system: Option<&str>, mut messages: Vec<Message>) -> Vec<Message> {
    let Some(system) = system else {
        return messages;
    };
    if messages.first().map(|first| first.role.as_str()) == Some("system") {
        return messages;
    }
    messages.insert(
        0,
        Message {
            role: "system".to_string(),
            content: system.to_string(),
        },
    );
    messages
}
