use llmff_core::error::LlmffError;

use super::batch_exit_code;

pub fn exit_code(error: &anyhow::Error) -> i32 {
    for cause in error.chain() {
        if let Some(error) = cause.downcast_ref::<LlmffError>() {
            return llmff_exit_code(error);
        }
    }
    if let Some(code) = batch_exit_code(error) {
        return code;
    }

    let message = error.to_string();
    if is_cli_usage_error(&message) {
        2
    } else if message.starts_with("one or more batch items failed") {
        20
    } else {
        1
    }
}

pub(super) fn pre_run_exit_code(error: &anyhow::Error) -> i32 {
    if let Some(code) = batch_exit_code(error) {
        return code;
    }
    let message = error.to_string();
    if is_cli_usage_error(&message) {
        2
    } else {
        1
    }
}

pub(super) fn llmff_exit_code(error: &LlmffError) -> i32 {
    match error {
        LlmffError::ManifestParse(_)
        | LlmffError::GraphValidation(_)
        | LlmffError::UnknownStage(_)
        | LlmffError::Config(_) => 10,
        LlmffError::StageExecution { message, .. }
            if message == "stage timed out" || message.starts_with("http tool ") =>
        {
            21
        }
        LlmffError::StageExecution { .. } => 20,
        LlmffError::Backend(_) => 21,
        LlmffError::Io(_) | LlmffError::Json(_) => 22,
        LlmffError::NotImplemented(_) => 30,
    }
}

fn is_cli_usage_error(message: &str) -> bool {
    [
        "provide either manifest or --graph",
        "stream-stage cannot write to stdout",
        "events cannot stream to stdout",
        "--run-dir owns trace, events, and checkpoint paths",
        "max-concurrency must be greater than 0",
        "timeout-ms must be greater than 0",
        "retry-attempts must be greater than 0",
        "batch mode does not support explicit trace, events, checkpoint, resume, replay-trace, or stream-stage flags",
        "batch mode requires",
        "batch mode output paths cannot contain parent directory components",
        "expected alias=value",
        "expected non-empty alias",
        "expected non-empty value",
    ]
    .iter()
    .any(|prefix| message.starts_with(prefix))
}
