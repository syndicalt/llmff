use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use serde::Serialize;

use crate::error::LlmffError;

#[derive(Debug, Clone, Serialize)]
pub struct TraceEvent {
    pub run_id: String,
    pub event: String,
    pub stage_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loop_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loop_iteration: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loop_stage_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub map_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub map_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub map_stage_id: Option<String>,
    pub op: Option<String>,
    pub status: Option<String>,
    pub timestamp_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u128>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attempts: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation_errors: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_hit: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_message: Option<String>,
}

pub struct TraceWriter {
    writer: BufWriter<Box<dyn Write + Send>>,
}

impl TraceWriter {
    pub fn create(path: &Path) -> Result<Self, LlmffError> {
        Ok(Self {
            writer: BufWriter::new(Box::new(File::create(path)?)),
        })
    }

    pub fn stdout() -> Self {
        Self {
            writer: BufWriter::new(Box::new(io::stdout())),
        }
    }

    pub fn write_event(&mut self, event: &TraceEvent) -> Result<(), LlmffError> {
        serde_json::to_writer(&mut self.writer, event)?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()?;
        Ok(())
    }
}
