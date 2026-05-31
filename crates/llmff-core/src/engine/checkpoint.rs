use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::LlmffError;
use crate::manifest::Manifest;
use crate::value::StageStatus;

const CHECKPOINT_RECORD_VERSION: u32 = 1;

#[derive(Debug, Deserialize, Serialize)]
struct CheckpointRecord {
    version: u32,
    manifest_hash: String,
    statuses: BTreeMap<String, StageStatus>,
}

pub(super) fn read_checkpoint(
    path: &Path,
    expected_manifest_hash: &str,
) -> Result<BTreeMap<String, StageStatus>, LlmffError> {
    let source = std::fs::read_to_string(path)?;
    let record: CheckpointRecord = serde_json::from_str(&source)?;
    if record.version != CHECKPOINT_RECORD_VERSION {
        return Err(LlmffError::Config(format!(
            "unsupported checkpoint version {}",
            record.version
        )));
    }
    if record.manifest_hash != expected_manifest_hash {
        return Err(LlmffError::Config(format!(
            "checkpoint manifest hash does not match current manifest: checkpoint={} checkpoint_hash={} current_manifest_hash={}; run inspect --format json on the manifest used for this run and resume only from a checkpoint produced by the same manifest",
            path.display(),
            record.manifest_hash,
            expected_manifest_hash
        )));
    }

    Ok(record.statuses)
}

pub(super) fn write_checkpoint_if_configured(
    checkpoint_path: Option<&Path>,
    statuses: &BTreeMap<String, StageStatus>,
    manifest_hash: &str,
) -> Result<(), LlmffError> {
    let Some(path) = checkpoint_path else {
        return Ok(());
    };
    let record = CheckpointRecord {
        version: CHECKPOINT_RECORD_VERSION,
        manifest_hash: manifest_hash.to_string(),
        statuses: statuses.clone(),
    };
    let encoded = serde_json::to_vec_pretty(&record).map_err(LlmffError::Json)?;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let tmp_file = path.with_extension(format!(
        "tmp.{}.{}",
        std::process::id(),
        super::timestamp_ms()
    ));
    std::fs::write(&tmp_file, encoded)?;
    std::fs::rename(&tmp_file, path)?;

    Ok(())
}

pub(super) fn manifest_fingerprint(manifest: &Manifest) -> Result<String, LlmffError> {
    let encoded = serde_json::to_vec(manifest).map_err(LlmffError::Json)?;
    let mut hasher = Sha256::new();
    hasher.update(encoded);
    Ok(format!("{:x}", hasher.finalize()))
}

pub(super) fn validate_replay_trace(path: &Path, has_checkpoint: bool) -> Result<(), LlmffError> {
    let source = std::fs::read_to_string(path)?;
    let mut has_stage_finished = false;
    for (index, line) in source.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let event: serde_json::Value = serde_json::from_str(line).map_err(|error| {
            LlmffError::Config(format!(
                "invalid replay trace JSON on line {}: {error}",
                index + 1
            ))
        })?;
        if event.get("event").and_then(serde_json::Value::as_str) == Some("stage_finished") {
            has_stage_finished = true;
        }
    }
    if !has_stage_finished {
        return Err(LlmffError::Config(
            "replay trace does not contain completed stages".to_string(),
        ));
    }

    if !has_checkpoint {
        return Err(LlmffError::Config(
            "trace replay requires a checkpoint because traces intentionally omit stage payloads"
                .to_string(),
        ));
    }

    Ok(())
}
