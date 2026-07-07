use std::path::Path;

use crate::error::LlmffError;
use crate::trace::{TraceEvent, TraceWriter};

use super::RunOptions;

pub(super) fn create_trace_writers(options: &RunOptions) -> Result<Vec<TraceWriter>, LlmffError> {
    let mut writers = Vec::new();

    if let Some(path) = options.trace_path.as_ref() {
        writers.push(TraceWriter::create(path)?);
    }

    if let Some(path) = options.event_path.as_ref() {
        if path == Path::new("-") {
            writers.push(TraceWriter::stdout());
        } else {
            writers.push(TraceWriter::create(path)?);
        }
    }

    Ok(writers)
}

pub(super) fn write_trace(
    trace: &mut Vec<TraceWriter>,
    event: TraceEvent,
) -> Result<(), LlmffError> {
    for trace in trace {
        trace.write_event(&event)?;
    }
    Ok(())
}

pub(super) fn write_run_failed(
    trace: &mut Vec<TraceWriter>,
    run_id: &str,
    error: &LlmffError,
) -> Result<(), LlmffError> {
    write_trace(
        trace,
        TraceEvent {
            run_id: run_id.to_string(),
            event: "run_failed".to_string(),
            agent: None,
            stage_id: failure_stage_id(error),
            loop_id: failure_loop_id(error),
            loop_iteration: failure_loop_iteration(error),
            loop_stage_id: failure_loop_stage_id(error),
            map_id: None,
            map_index: None,
            map_stage_id: None,
            op: None,
            status: Some("failed".to_string()),
            timestamp_ms: super::timestamp_ms(),
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
            failure_kind: Some(failure_kind(error).to_string()),
            failure_message: Some(failure_message(error).to_string()),
        },
    )
}

fn failure_stage_id(error: &LlmffError) -> Option<String> {
    match error {
        LlmffError::StageExecution { stage_id, .. } => Some(stage_id.clone()),
        LlmffError::LoopStageExecution { stage_id, .. } => Some(stage_id.clone()),
        _ => None,
    }
}

fn failure_loop_id(error: &LlmffError) -> Option<String> {
    match error {
        LlmffError::LoopStageExecution { loop_id, .. } => Some(loop_id.clone()),
        _ => None,
    }
}

fn failure_loop_iteration(error: &LlmffError) -> Option<usize> {
    match error {
        LlmffError::LoopStageExecution { loop_iteration, .. } => Some(*loop_iteration),
        _ => None,
    }
}

fn failure_loop_stage_id(error: &LlmffError) -> Option<String> {
    match error {
        LlmffError::LoopStageExecution { loop_stage_id, .. } => Some(loop_stage_id.clone()),
        _ => None,
    }
}

fn failure_kind(error: &LlmffError) -> &'static str {
    error.failure_kind().as_str()
}

fn failure_message(error: &LlmffError) -> &'static str {
    error.failure_message()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::HttpToolFailure;

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

    // Characterization cases for `failure_kind` / `failure_message` / Display.
    // Expected values are frozen: captured from the unmodified implementation
    // before the typed refactor. Do not edit them.
    #[test]
    fn manifest_parse_case() {
        let error = manifest_parse_error();
        assert_eq!(failure_kind(&error), "manifest_parse");
        assert_eq!(failure_message(&error), "manifest parse failed");
        assert!(error.to_string().starts_with("failed to parse manifest: "));
    }

    #[test]
    fn io_case() {
        let error = io_error();
        assert_eq!(failure_kind(&error), "io");
        assert_eq!(failure_message(&error), "I/O operation failed");
        assert!(error.to_string().starts_with("I/O error: "));
    }

    #[test]
    fn json_case() {
        let error = json_error();
        assert_eq!(failure_kind(&error), "json");
        assert_eq!(failure_message(&error), "JSON operation failed");
        assert!(error.to_string().starts_with("JSON error: "));
    }

    #[test]
    fn graph_validation_case() {
        let error = LlmffError::GraphValidation("boom".to_string());
        assert_eq!(failure_kind(&error), "graph_validation");
        assert_eq!(failure_message(&error), "graph validation failed");
        assert_eq!(error.to_string(), "graph validation failed: boom");
    }

    #[test]
    fn unknown_stage_case() {
        let error = LlmffError::UnknownStage("weird_op".to_string());
        assert_eq!(failure_kind(&error), "unknown_stage");
        assert_eq!(failure_message(&error), "unknown stage operation");
        assert_eq!(error.to_string(), "unknown stage operation `weird_op`");
    }

    #[test]
    fn config_case() {
        let error = LlmffError::Config("bad".to_string());
        assert_eq!(failure_kind(&error), "config");
        assert_eq!(failure_message(&error), "configuration failed");
        assert_eq!(error.to_string(), "configuration error: bad");
    }

    #[test]
    fn stage_timeout_case() {
        let error = LlmffError::StageTimeout {
            stage_id: "s1".to_string(),
        };
        assert_eq!(failure_kind(&error), "timeout");
        assert_eq!(failure_message(&error), "stage timed out");
        assert_eq!(error.to_string(), "stage `s1` failed: stage timed out");
    }

    #[test]
    fn http_tool_requires_method_case() {
        let error = LlmffError::HttpTool {
            stage_id: "s1".to_string(),
            kind: HttpToolFailure::RequiresMethod,
        };
        assert_eq!(failure_kind(&error), "http");
        assert_eq!(failure_message(&error), "HTTP stage failed");
        assert_eq!(
            error.to_string(),
            "stage `s1` failed: http tool requires method"
        );
    }

    #[test]
    fn http_tool_requires_url_case() {
        let error = LlmffError::HttpTool {
            stage_id: "s1".to_string(),
            kind: HttpToolFailure::RequiresUrl,
        };
        assert_eq!(failure_kind(&error), "http");
        assert_eq!(failure_message(&error), "HTTP stage failed");
        assert_eq!(
            error.to_string(),
            "stage `s1` failed: http tool requires url"
        );
    }

    #[test]
    fn http_tool_request_failed_case() {
        let error = LlmffError::HttpTool {
            stage_id: "s1".to_string(),
            kind: HttpToolFailure::RequestFailed("connection reset".to_string()),
        };
        assert_eq!(failure_kind(&error), "http");
        assert_eq!(failure_message(&error), "HTTP stage failed");
        assert_eq!(
            error.to_string(),
            "stage `s1` failed: http tool request failed: connection reset"
        );
    }

    #[test]
    fn http_tool_status_error_case() {
        let error = LlmffError::HttpTool {
            stage_id: "s1".to_string(),
            kind: HttpToolFailure::Status {
                status_code: 500,
                status_text: "500 Internal Server Error".to_string(),
                body: "boom".to_string(),
            },
        };
        assert_eq!(failure_kind(&error), "http");
        assert_eq!(failure_message(&error), "HTTP stage failed");
        assert_eq!(
            error.to_string(),
            "stage `s1` failed: http tool returned status 500 Internal Server Error: boom"
        );
    }

    // Gap cases: today these do not match the "http tool " prefix, so they
    // classify as generic stage_execution, not http. The refactor must
    // preserve this exact split.
    #[test]
    fn invalid_http_tool_method_case() {
        let error = LlmffError::StageExecution {
            stage_id: "s1".to_string(),
            message: "invalid http tool method: bad method".to_string(),
        };
        assert_eq!(failure_kind(&error), "stage_execution");
        assert_eq!(failure_message(&error), "stage execution failed");
        assert_eq!(
            error.to_string(),
            "stage `s1` failed: invalid http tool method: bad method"
        );
    }

    #[test]
    fn failed_to_read_http_tool_response_case() {
        let error = LlmffError::StageExecution {
            stage_id: "s1".to_string(),
            message: "failed to read http tool response: broken pipe".to_string(),
        };
        assert_eq!(failure_kind(&error), "stage_execution");
        assert_eq!(failure_message(&error), "stage execution failed");
        assert_eq!(
            error.to_string(),
            "stage `s1` failed: failed to read http tool response: broken pipe"
        );
    }

    #[test]
    fn generic_stage_execution_case() {
        let error = LlmffError::StageExecution {
            stage_id: "s1".to_string(),
            message: "tool command exited with status 1".to_string(),
        };
        assert_eq!(failure_kind(&error), "stage_execution");
        assert_eq!(failure_message(&error), "stage execution failed");
        assert_eq!(
            error.to_string(),
            "stage `s1` failed: tool command exited with status 1"
        );
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
        assert_eq!(failure_kind(&error), "timeout");
        assert_eq!(failure_message(&error), "stage timed out");
        assert_eq!(
            error.to_string(),
            "loop body stage `loop.s1` failed: stage `s1` failed: stage timed out"
        );
    }

    #[test]
    fn backend_case() {
        let error = LlmffError::Backend("down".to_string());
        assert_eq!(failure_kind(&error), "backend");
        assert_eq!(failure_message(&error), "backend request failed");
        assert_eq!(error.to_string(), "backend error: down");
    }

    #[test]
    fn not_implemented_case() {
        let error = LlmffError::NotImplemented("feature");
        assert_eq!(failure_kind(&error), "not_implemented");
        assert_eq!(failure_message(&error), "feature is not implemented");
        assert_eq!(error.to_string(), "not implemented: feature");
    }
}
