use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::io::AsyncWriteExt as _;
use tokio::process::Command;

use crate::error::{HttpToolFailure, LlmffError};
use crate::manifest::StageSpec;
use crate::plugin::PluginToolTransport;
use crate::stage::specs::{ToolSpec, ToolTransport};
use crate::value::{StageStatus, Value};

use super::{render_messages_as_text, retry_policy, Engine, RetryPolicy, StageOutcome};

struct ToolAttemptResult {
    status: StageStatus,
    attempts: usize,
}

impl Engine {
    pub(super) async fn execute_tool(
        &self,
        stage: &StageSpec,
        statuses: &BTreeMap<String, StageStatus>,
        cwd: &Path,
        plugin_tool_transports: &BTreeMap<String, PluginToolTransport>,
        default_retry: RetryPolicy,
    ) -> Result<StageOutcome, LlmffError> {
        let typed = ToolSpec::parse(stage).map_err(|message| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message,
        })?;

        match typed.transport {
            ToolTransport::Command(command) => execute_command_tool(stage, statuses, cwd, &command)
                .await
                .map(StageOutcome::without_usage),
            ToolTransport::Http => {
                let result = execute_http_tool_with_retry(stage, statuses, default_retry).await?;
                Ok(StageOutcome::with_usage_attempts(
                    result.status,
                    None,
                    result.attempts,
                ))
            }
            ToolTransport::Plugin(transport) => {
                let plugin_transport = plugin_tool_transports.get(&transport).ok_or_else(|| {
                    LlmffError::StageExecution {
                        stage_id: stage.id.clone(),
                        message: format!("unknown plugin tool transport `{transport}`"),
                    }
                })?;
                execute_command_tool(
                    stage,
                    statuses,
                    cwd,
                    &[plugin_transport.entrypoint.to_string_lossy().into_owned()],
                )
                .await
                .map(StageOutcome::without_usage)
            }
        }
    }
}

async fn execute_command_tool(
    stage: &StageSpec,
    statuses: &BTreeMap<String, StageStatus>,
    cwd: &Path,
    command: &[String],
) -> Result<StageStatus, LlmffError> {
    execute_command_stage(stage, statuses, cwd, command, "tool command").await
}

pub(super) async fn execute_command_stage(
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
        .ok_or_else(|| LlmffError::HttpTool {
            stage_id: stage.id.clone(),
            kind: HttpToolFailure::RequiresMethod,
        })?
        .parse::<reqwest::Method>()
        .map_err(|error| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!("invalid http tool method: {error}"),
        })?;
    let url = stage.url.as_deref().ok_or_else(|| LlmffError::HttpTool {
        stage_id: stage.id.clone(),
        kind: HttpToolFailure::RequiresUrl,
    })?;

    let client = reqwest::Client::new();
    let mut request = client.request(method.clone(), url);
    for (name, value) in &stage.headers {
        request = request.header(name, value);
    }
    if method_allows_body(&method) {
        request = request.body(input);
    }

    let response = request.send().await.map_err(|error| LlmffError::HttpTool {
        stage_id: stage.id.clone(),
        kind: HttpToolFailure::RequestFailed(error.to_string()),
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
        return Err(LlmffError::HttpTool {
            stage_id: stage.id.clone(),
            kind: HttpToolFailure::Status {
                status_code: status.as_u16(),
                status_text: status.to_string(),
                body,
            },
        });
    }

    Ok(StageStatus::Success(Value::Text(body)))
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
    match error {
        LlmffError::HttpTool { kind, .. } => kind.is_retryable(),
        _ => false,
    }
}

pub(super) async fn sleep_for_retry(backoff_ms: u64) {
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
