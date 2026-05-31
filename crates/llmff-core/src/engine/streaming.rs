use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use crate::error::LlmffError;
use crate::graph::Graph;
use crate::manifest::StageSpec;
use crate::value::{StageStatus, Value};

use super::{resolve_path_buf, serialize_value, RunOptions, StageOutcome};

pub(super) struct StageStreamWriter {
    pub(super) stage_id: String,
    writer: BufWriter<Box<dyn Write + Send>>,
}

impl StageStreamWriter {
    fn stdout(stage_id: String) -> Self {
        Self {
            stage_id,
            writer: BufWriter::new(Box::new(std::io::stdout())),
        }
    }

    fn create(stage_id: String, path: &Path) -> Result<Self, LlmffError> {
        Ok(Self {
            stage_id,
            writer: BufWriter::new(Box::new(File::create(path)?)),
        })
    }

    pub(super) fn write_delta(&mut self, delta: &str) -> Result<(), LlmffError> {
        self.writer.write_all(delta.as_bytes())?;
        self.writer.flush()?;
        Ok(())
    }

    fn write_value(&mut self, value: &Value) -> Result<(), LlmffError> {
        self.writer.write_all(serialize_value(value)?.as_bytes())?;
        self.writer.flush()?;
        Ok(())
    }
}

pub(super) fn stream_stage_payload_if_selected(
    stream_writer: Option<&mut StageStreamWriter>,
    stage: &StageSpec,
    outcome: &StageOutcome,
) -> Result<(), LlmffError> {
    let Some(writer) = stream_writer else {
        return Ok(());
    };
    if writer.stage_id != stage.id || outcome.stream_written {
        return Ok(());
    }
    match &outcome.status {
        StageStatus::Success(value) | StageStatus::Invalid { value, .. } => {
            writer.write_value(value)
        }
        StageStatus::Skipped => Ok(()),
    }
}

pub(super) fn create_stage_stream_writer(
    options: &RunOptions,
    graph: &Graph,
    cwd: &Path,
) -> Result<Option<StageStreamWriter>, LlmffError> {
    let Some(stage_id) = options.stream_stage.as_ref() else {
        return Ok(None);
    };
    graph
        .stages()
        .iter()
        .find(|stage| stage.id == *stage_id)
        .ok_or_else(|| {
            LlmffError::Config(format!(
                "stream-stage references unknown stage `{stage_id}`"
            ))
        })?;

    let path = options
        .stream_path
        .as_deref()
        .unwrap_or_else(|| Path::new("-"));
    if path == Path::new("-") {
        Ok(Some(StageStreamWriter::stdout(stage_id.clone())))
    } else {
        Ok(Some(StageStreamWriter::create(
            stage_id.clone(),
            &resolve_path_buf(cwd, path),
        )?))
    }
}
