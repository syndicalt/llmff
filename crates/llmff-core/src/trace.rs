use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use serde::Serialize;

use crate::error::LlmffError;

#[derive(Debug, Clone, Serialize)]
pub struct TraceEvent {
    pub run_id: String,
    pub event: String,
    pub stage_id: Option<String>,
    pub op: Option<String>,
    pub status: Option<String>,
}

pub struct TraceWriter {
    writer: BufWriter<File>,
}

impl TraceWriter {
    pub fn create(path: &Path) -> Result<Self, LlmffError> {
        Ok(Self {
            writer: BufWriter::new(File::create(path)?),
        })
    }

    pub fn write_event(&mut self, event: &TraceEvent) -> Result<(), LlmffError> {
        serde_json::to_writer(&mut self.writer, event)?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()?;
        Ok(())
    }
}
