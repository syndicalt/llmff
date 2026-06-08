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
    match error {
        LlmffError::ManifestParse(_) => "manifest_parse",
        LlmffError::Io(_) => "io",
        LlmffError::Json(_) => "json",
        LlmffError::GraphValidation(_) => "graph_validation",
        LlmffError::UnknownStage(_) => "unknown_stage",
        LlmffError::StageExecution { message, .. } if message == "stage timed out" => "timeout",
        LlmffError::StageExecution { message, .. } if message.starts_with("http tool ") => "http",
        LlmffError::StageExecution { .. } => "stage_execution",
        LlmffError::LoopStageExecution { source, .. } => failure_kind(source),
        LlmffError::Backend(_) => "backend",
        LlmffError::Config(_) => "config",
        LlmffError::NotImplemented(_) => "not_implemented",
    }
}

fn failure_message(error: &LlmffError) -> &'static str {
    match error {
        LlmffError::ManifestParse(_) => "manifest parse failed",
        LlmffError::Io(_) => "I/O operation failed",
        LlmffError::Json(_) => "JSON operation failed",
        LlmffError::GraphValidation(_) => "graph validation failed",
        LlmffError::UnknownStage(_) => "unknown stage operation",
        LlmffError::StageExecution { message, .. } if message == "stage timed out" => {
            "stage timed out"
        }
        LlmffError::StageExecution { message, .. } if message.starts_with("http tool ") => {
            "HTTP stage failed"
        }
        LlmffError::StageExecution { .. } => "stage execution failed",
        LlmffError::LoopStageExecution { source, .. } => failure_message(source),
        LlmffError::Backend(_) => "backend request failed",
        LlmffError::Config(_) => "configuration failed",
        LlmffError::NotImplemented(_) => "feature is not implemented",
    }
}
