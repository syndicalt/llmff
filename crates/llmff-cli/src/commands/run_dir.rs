use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use llmff_core::error::LlmffError;
use llmff_core::manifest::Manifest;
use llmff_core::value::StageStatus;
use sha2::{Digest, Sha256};

use super::batch::BatchRunError;
use super::exit_codes::{llmff_exit_code, pre_run_exit_code};
use super::load_pipeline_manifest;

#[derive(Debug)]
pub(super) struct RunDirArtifacts {
    pub(super) inspect_path: PathBuf,
    pub(super) trace_path: PathBuf,
    pub(super) events_path: PathBuf,
    pub(super) checkpoint_path: PathBuf,
    pub(super) result_path: PathBuf,
}

impl RunDirArtifacts {
    pub(super) fn new(run_dir: &Path) -> Result<Self> {
        fs::create_dir_all(run_dir)?;
        Ok(Self {
            inspect_path: run_dir.join("inspect.json"),
            trace_path: run_dir.join("trace.jsonl"),
            events_path: run_dir.join("events.jsonl"),
            checkpoint_path: run_dir.join("checkpoint.json"),
            result_path: run_dir.join("result.json"),
        })
    }
}

pub(super) fn run_result_summary_for_llmff_error(
    manifest_hash: &str,
    artifacts: &RunDirArtifacts,
    error: Option<&LlmffError>,
) -> serde_json::Value {
    let failure = error.map(|error| {
        serde_json::json!({
            "kind": failure_kind(error),
            "message": error.to_string(),
            "retry_recommendation": retry_recommendation(error),
        })
    });
    serde_json::json!({
        "schema_version": 1,
        "status": if error.is_some() { "failed" } else { "succeeded" },
        "exit_code": error.map(llmff_exit_code).unwrap_or(0),
        "manifest": {
            "hash": format!("sha256:{manifest_hash}"),
        },
        "artifacts": {
            "inspect": relative_artifact_name(&artifacts.inspect_path),
            "trace": relative_artifact_name(&artifacts.trace_path),
            "events": relative_artifact_name(&artifacts.events_path),
            "checkpoint": relative_artifact_name(&artifacts.checkpoint_path),
        },
        "failure": failure,
    })
}

pub(super) fn run_result_summary_for_error(
    manifest_hash: &str,
    artifacts: &RunDirArtifacts,
    error: &anyhow::Error,
) -> serde_json::Value {
    run_result_summary_for_error_result(manifest_hash, artifacts, Some(error))
}

pub(super) fn run_result_summary_for_error_result(
    manifest_hash: &str,
    artifacts: &RunDirArtifacts,
    error: Option<&anyhow::Error>,
) -> serde_json::Value {
    let Some(error) = error else {
        return run_result_summary_for_llmff_error(manifest_hash, artifacts, None);
    };

    for cause in error.chain() {
        if let Some(error) = cause.downcast_ref::<LlmffError>() {
            return run_result_summary_for_llmff_error(manifest_hash, artifacts, Some(error));
        }
        if let Some(error) = cause.downcast_ref::<BatchRunError>() {
            return serde_json::json!({
                "schema_version": 1,
                "status": "failed",
                "exit_code": error.exit_code,
                "manifest": {
                    "hash": format!("sha256:{manifest_hash}"),
                },
                "artifacts": {
                    "inspect": relative_artifact_name(&artifacts.inspect_path),
                    "trace": relative_artifact_name(&artifacts.trace_path),
                    "events": relative_artifact_name(&artifacts.events_path),
                    "checkpoint": relative_artifact_name(&artifacts.checkpoint_path),
                },
                "failure": {
                    "kind": error.failure_kind,
                    "message": error.to_string(),
                    "retry_recommendation": error.retry_recommendation,
                },
            });
        }
    }

    if error
        .to_string()
        .starts_with("one or more batch items failed")
    {
        return serde_json::json!({
            "schema_version": 1,
            "status": "failed",
            "exit_code": 20,
            "manifest": {
                "hash": format!("sha256:{manifest_hash}"),
            },
            "artifacts": {
                "inspect": relative_artifact_name(&artifacts.inspect_path),
                "trace": relative_artifact_name(&artifacts.trace_path),
                "events": relative_artifact_name(&artifacts.events_path),
                "checkpoint": relative_artifact_name(&artifacts.checkpoint_path),
            },
            "failure": {
                "kind": "stage_execution",
                "message": error.to_string(),
                "retry_recommendation": "check_stage_or_input",
            },
        });
    }

    serde_json::json!({
        "schema_version": 1,
        "status": "failed",
        "exit_code": pre_run_exit_code(error),
        "manifest": {
            "hash": format!("sha256:{manifest_hash}"),
        },
        "artifacts": {
            "inspect": relative_artifact_name(&artifacts.inspect_path),
            "trace": relative_artifact_name(&artifacts.trace_path),
            "events": relative_artifact_name(&artifacts.events_path),
            "checkpoint": relative_artifact_name(&artifacts.checkpoint_path),
        },
        "failure": {
            "kind": "config",
            "message": error.to_string(),
            "retry_recommendation": "do_not_retry_without_changes",
        },
    })
}

pub(super) fn initialize_batch_run_dir_artifacts(
    artifacts: &RunDirArtifacts,
    manifest: &Manifest,
) -> Result<()> {
    fs::File::create(&artifacts.trace_path)?;
    write_batch_checkpoint(
        artifacts,
        &manifest_fingerprint(manifest)?,
        &BTreeMap::new(),
    )?;

    let mut events = fs::File::create(&artifacts.events_path)?;
    writeln!(
        events,
        "{}",
        serde_json::json!({
            "run_id": "cli-batch",
            "event": "run_started",
            "timestamp_ms": timestamp_ms(),
        })
    )?;
    Ok(())
}

pub(super) fn finish_batch_run_dir_events(
    artifacts: &RunDirArtifacts,
    error: Option<&anyhow::Error>,
) -> Result<()> {
    let mut events = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&artifacts.events_path)?;
    let event = if let Some(error) = error {
        let (kind, message) = run_dir_anyhow_failure(error);
        serde_json::json!({
            "run_id": "cli-batch",
            "event": "run_failed",
            "status": "failed",
            "timestamp_ms": timestamp_ms(),
            "failure_kind": kind,
            "failure_message": message,
        })
    } else {
        serde_json::json!({
            "run_id": "cli-batch",
            "event": "run_finished",
            "status": "success",
            "timestamp_ms": timestamp_ms(),
        })
    };
    writeln!(events, "{event}")?;
    Ok(())
}

pub(super) fn write_batch_trace_event(
    artifacts: Option<&RunDirArtifacts>,
    event: serde_json::Value,
) -> Result<()> {
    let Some(artifacts) = artifacts else {
        return Ok(());
    };
    let mut trace = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&artifacts.trace_path)?;
    writeln!(trace, "{event}")?;
    Ok(())
}

pub(super) fn write_batch_checkpoint(
    artifacts: &RunDirArtifacts,
    manifest_hash: &str,
    statuses: &BTreeMap<String, StageStatus>,
) -> Result<()> {
    let checkpoint = serde_json::json!({
        "version": 1,
        "manifest_hash": manifest_hash,
        "statuses": statuses,
    });
    write_json_file(&artifacts.checkpoint_path, &checkpoint)
}

pub(super) fn append_interrupted_run_event(artifacts: &RunDirArtifacts) -> Result<()> {
    let mut events = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&artifacts.events_path)?;
    writeln!(
        events,
        "{}",
        serde_json::json!({
            "run_id": "cli-run",
            "event": "run_failed",
            "status": "failed",
            "timestamp_ms": timestamp_ms(),
            "failure_kind": "interrupted",
            "failure_message": "interrupted",
        })
    )?;
    Ok(())
}

pub(super) fn interrupted_run_result_summary(
    manifest_hash: &str,
    artifacts: &RunDirArtifacts,
) -> serde_json::Value {
    serde_json::json!({
        "schema_version": 1,
        "status": "failed",
        "exit_code": 130,
        "manifest": {
            "hash": format!("sha256:{manifest_hash}"),
        },
        "artifacts": {
            "inspect": relative_artifact_name(&artifacts.inspect_path),
            "trace": relative_artifact_name(&artifacts.trace_path),
            "events": relative_artifact_name(&artifacts.events_path),
            "checkpoint": relative_artifact_name(&artifacts.checkpoint_path),
        },
        "failure": {
            "kind": "interrupted",
            "message": "interrupted",
            "retry_recommendation": "resume_with_matching_checkpoint",
        },
    })
}

pub(super) fn manifest_fingerprint(manifest: &Manifest) -> Result<String> {
    let encoded = serde_json::to_vec(manifest)?;
    Ok(sha256_hex(&encoded))
}

pub(super) fn timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

pub(super) fn manifest_bytes_for_failure(
    manifest_path: Option<&PathBuf>,
    inline_graph: Option<&String>,
) -> Vec<u8> {
    if let Some(graph) = inline_graph {
        return graph.as_bytes().to_vec();
    }
    manifest_path
        .and_then(|path| fs::read(path).ok())
        .unwrap_or_default()
}

pub(super) fn manifest_hash_for_interrupt(
    manifest_path: Option<&PathBuf>,
    inline_graph: Option<&String>,
) -> String {
    let Ok(loaded) = load_pipeline_manifest(manifest_path.cloned(), None, inline_graph.cloned())
    else {
        return sha256_hex(&manifest_bytes_for_failure(manifest_path, inline_graph));
    };
    manifest_fingerprint(&loaded.manifest)
        .unwrap_or_else(|_| sha256_hex(loaded.source.content.as_bytes()))
}

pub(super) fn failure_kind(error: &LlmffError) -> &'static str {
    match error {
        LlmffError::ManifestParse(_) => "manifest_parse",
        LlmffError::GraphValidation(_) => "graph_validation",
        LlmffError::UnknownStage(_) => "unknown_stage",
        LlmffError::Config(_) => "config",
        LlmffError::StageExecution { message, .. } if message == "stage timed out" => "timeout",
        LlmffError::StageExecution { message, .. } if message.starts_with("http tool ") => "http",
        LlmffError::StageExecution { .. } => "stage_execution",
        LlmffError::LoopStageExecution { source, .. } => failure_kind(source),
        LlmffError::Backend(_) => "backend",
        LlmffError::Io(_) => "io",
        LlmffError::Json(_) => "json",
        LlmffError::NotImplemented(_) => "not_implemented",
    }
}

pub(super) fn retry_recommendation(error: &LlmffError) -> &'static str {
    match error {
        LlmffError::Backend(_) => "retry_with_backoff",
        LlmffError::StageExecution { .. } => "check_stage_or_input",
        LlmffError::LoopStageExecution { source, .. } => retry_recommendation(source),
        LlmffError::Io(_) => "check_filesystem",
        _ => "do_not_retry_without_changes",
    }
}

pub(super) fn write_json_file(path: &Path, value: &serde_json::Value) -> Result<()> {
    fs::write(path, format!("{}\n", serde_json::to_string_pretty(value)?))?;
    Ok(())
}

fn run_dir_anyhow_failure(error: &anyhow::Error) -> (&'static str, String) {
    for cause in error.chain() {
        if let Some(error) = cause.downcast_ref::<LlmffError>() {
            return (failure_kind(error), error.to_string());
        }
        if let Some(error) = cause.downcast_ref::<BatchRunError>() {
            return (error.failure_kind, error.to_string());
        }
    }
    if error
        .to_string()
        .starts_with("one or more batch items failed")
    {
        ("stage_execution", error.to_string())
    } else {
        ("config", error.to_string())
    }
}

fn relative_artifact_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
