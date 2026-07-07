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
        LlmffError::StageTimeout { .. } | LlmffError::HttpTool { .. } => 21,
        LlmffError::StageExecution { .. } => 20,
        LlmffError::LoopStageExecution { source, .. } => llmff_exit_code(source),
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

#[cfg(test)]
mod tests {
    use super::*;
    use llmff_core::error::HttpToolFailure;

    fn manifest_parse_error() -> LlmffError {
        let error = serde_yaml::from_str::<serde_yaml::Value>("key: [unterminated")
            .expect_err("malformed yaml must fail to parse");
        LlmffError::ManifestParse(error)
    }

    fn io_error() -> LlmffError {
        LlmffError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "missing"))
    }

    fn json_error() -> LlmffError {
        let error = serde_json::from_str::<serde_json::Value>("{")
            .expect_err("malformed json must fail to parse");
        LlmffError::Json(error)
    }

    // Characterization cases for `llmff_exit_code`. Each case constructs an
    // `LlmffError` the way production code does and freezes the resulting
    // exit code. Do not edit expected values; they were captured from the
    // unmodified implementation before the typed refactor.
    #[test]
    fn manifest_parse_exits_10() {
        assert_eq!(llmff_exit_code(&manifest_parse_error()), 10);
    }

    #[test]
    fn graph_validation_exits_10() {
        assert_eq!(
            llmff_exit_code(&LlmffError::GraphValidation("boom".to_string())),
            10
        );
    }

    #[test]
    fn unknown_stage_exits_10() {
        assert_eq!(
            llmff_exit_code(&LlmffError::UnknownStage("weird_op".to_string())),
            10
        );
    }

    #[test]
    fn config_exits_10() {
        assert_eq!(llmff_exit_code(&LlmffError::Config("bad".to_string())), 10);
    }

    #[test]
    fn stage_timeout_exits_21() {
        let error = LlmffError::StageTimeout {
            stage_id: "s1".to_string(),
        };
        assert_eq!(llmff_exit_code(&error), 21);
    }

    #[test]
    fn http_tool_requires_method_exits_21() {
        let error = LlmffError::HttpTool {
            stage_id: "s1".to_string(),
            kind: HttpToolFailure::RequiresMethod,
        };
        assert_eq!(llmff_exit_code(&error), 21);
    }

    #[test]
    fn http_tool_requires_url_exits_21() {
        let error = LlmffError::HttpTool {
            stage_id: "s1".to_string(),
            kind: HttpToolFailure::RequiresUrl,
        };
        assert_eq!(llmff_exit_code(&error), 21);
    }

    #[test]
    fn http_tool_request_failed_exits_21() {
        let error = LlmffError::HttpTool {
            stage_id: "s1".to_string(),
            kind: HttpToolFailure::RequestFailed("connection reset".to_string()),
        };
        assert_eq!(llmff_exit_code(&error), 21);
    }

    #[test]
    fn http_tool_status_error_exits_21() {
        let error = LlmffError::HttpTool {
            stage_id: "s1".to_string(),
            kind: HttpToolFailure::Status {
                status_code: 500,
                status_text: "500 Internal Server Error".to_string(),
                body: "boom".to_string(),
            },
        };
        assert_eq!(llmff_exit_code(&error), 21);
    }

    // Gap cases: these messages do not match the "http tool " prefix today,
    // so they fall through to the generic StageExecution exit code (20), not
    // 21. The refactor must preserve this exact split.
    #[test]
    fn invalid_http_tool_method_exits_20() {
        let error = LlmffError::StageExecution {
            stage_id: "s1".to_string(),
            message: "invalid http tool method: bad method".to_string(),
        };
        assert_eq!(llmff_exit_code(&error), 20);
    }

    #[test]
    fn failed_to_read_http_tool_response_exits_20() {
        let error = LlmffError::StageExecution {
            stage_id: "s1".to_string(),
            message: "failed to read http tool response: broken pipe".to_string(),
        };
        assert_eq!(llmff_exit_code(&error), 20);
    }

    #[test]
    fn generic_stage_execution_exits_20() {
        let error = LlmffError::StageExecution {
            stage_id: "s1".to_string(),
            message: "tool command exited with status 1".to_string(),
        };
        assert_eq!(llmff_exit_code(&error), 20);
    }

    #[test]
    fn loop_stage_execution_recurses_into_source() {
        let inner = LlmffError::StageTimeout {
            stage_id: "s1".to_string(),
        };
        let error = LlmffError::LoopStageExecution {
            stage_id: "loop.s1".to_string(),
            loop_id: "loop".to_string(),
            loop_iteration: 0,
            loop_stage_id: "s1".to_string(),
            source: Box::new(inner),
        };
        assert_eq!(llmff_exit_code(&error), 21);
    }

    #[test]
    fn backend_exits_21() {
        assert_eq!(
            llmff_exit_code(&LlmffError::Backend("down".to_string())),
            21
        );
    }

    #[test]
    fn io_exits_22() {
        assert_eq!(llmff_exit_code(&io_error()), 22);
    }

    #[test]
    fn json_exits_22() {
        assert_eq!(llmff_exit_code(&json_error()), 22);
    }

    #[test]
    fn not_implemented_exits_30() {
        assert_eq!(llmff_exit_code(&LlmffError::NotImplemented("feature")), 30);
    }
}
