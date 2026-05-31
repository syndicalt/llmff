use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use anyhow::Result;
use llmff_core::engine::{Engine, RetryPolicy, RunOptions, SchedulerMode};
use llmff_core::manifest::Manifest;
use llmff_core::value::{StageStatus, Value};

use super::exit_codes::llmff_exit_code;
use super::run_dir::{
    failure_kind, manifest_fingerprint, retry_recommendation, timestamp_ms, write_batch_checkpoint,
    write_batch_trace_event, RunDirArtifacts,
};

pub(super) struct BatchPipelineRequest<'a> {
    pub(super) manifest: Manifest,
    pub(super) cwd: &'a Path,
    pub(super) engine: &'a Engine,
    pub(super) batch_input: Option<PathBuf>,
    pub(super) batch_output_dir: Option<PathBuf>,
    pub(super) parallel: bool,
    pub(super) max_concurrency: Option<usize>,
    pub(super) timeout_ms: Option<u64>,
    pub(super) retry_attempts: Option<usize>,
    pub(super) retry_backoff_ms: Option<u64>,
    pub(super) plugin_dirs: Vec<PathBuf>,
    pub(super) run_dir_artifacts: Option<&'a RunDirArtifacts>,
}

pub(super) async fn run_batch_pipeline(request: BatchPipelineRequest<'_>) -> Result<()> {
    let BatchPipelineRequest {
        manifest,
        cwd,
        engine,
        batch_input,
        batch_output_dir,
        parallel,
        max_concurrency,
        timeout_ms,
        retry_attempts,
        retry_backoff_ms,
        plugin_dirs,
        run_dir_artifacts,
    } = request;

    let batch_input =
        batch_input.ok_or_else(|| anyhow::anyhow!("batch mode requires --batch-input"))?;
    let batch_output_dir = batch_output_dir
        .ok_or_else(|| anyhow::anyhow!("batch mode requires --batch-output-dir"))?;
    if manifest.inputs.len() != 1 {
        anyhow::bail!("batch mode requires a manifest with exactly one input");
    }
    if manifest
        .outputs
        .values()
        .any(|output| output.path.as_str() == "-")
    {
        anyhow::bail!("batch mode requires file outputs, not stdout outputs");
    }
    if max_concurrency == Some(0) {
        anyhow::bail!("max-concurrency must be greater than 0");
    }
    if timeout_ms == Some(0) {
        anyhow::bail!("timeout-ms must be greater than 0");
    }
    if retry_attempts == Some(0) {
        anyhow::bail!("retry-attempts must be greater than 0");
    }

    std::fs::create_dir_all(batch_output_dir.join("inputs"))?;
    std::fs::create_dir_all(batch_output_dir.join("items"))?;
    let report_path = batch_output_dir.join("batch-report.jsonl");
    let mut report = std::io::BufWriter::new(std::fs::File::create(&report_path)?);
    let batch_source = std::fs::read_to_string(&batch_input)?;
    let input_name = manifest
        .inputs
        .keys()
        .next()
        .expect("input length checked above")
        .clone();
    let default_retry = retry_attempts
        .map(|attempts| RetryPolicy {
            attempts,
            backoff_ms: retry_backoff_ms.unwrap_or(0),
        })
        .unwrap_or_default();
    let manifest_hash = manifest_fingerprint(&manifest)?;
    let mut checkpoint_statuses: BTreeMap<String, StageStatus> = BTreeMap::new();
    let mut batch_failure: Option<BatchRunError> = None;

    for (index, item) in batch_source.lines().enumerate() {
        let item_id = format!("{index:06}");
        let item_input_path = batch_output_dir
            .join("inputs")
            .join(format!("{item_id}.txt"));
        let item_output_dir = batch_output_dir.join("items").join(&item_id);
        std::fs::create_dir_all(&item_output_dir)?;
        std::fs::write(&item_input_path, item)?;

        let mut item_manifest = manifest.clone();
        item_manifest
            .inputs
            .get_mut(&input_name)
            .expect("input key should exist")
            .path = Some(item_input_path.to_string_lossy().into_owned());
        for output in item_manifest.outputs.values_mut() {
            let path = Path::new(&output.path);
            if path.is_absolute() {
                anyhow::bail!("batch mode requires relative output paths");
            }
            if path
                .components()
                .any(|component| matches!(component, Component::ParentDir))
            {
                anyhow::bail!("batch mode output paths cannot contain parent directory components");
            }
            output.path = item_output_dir.join(path).to_string_lossy().into_owned();
            if let Some(parent) = Path::new(&output.path).parent() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let options = RunOptions {
            run_id: format!("cli-batch-{item_id}"),
            scheduler: if parallel {
                SchedulerMode::Parallel
            } else {
                SchedulerMode::Sequential
            },
            plugin_dirs: plugin_dirs.clone(),
            max_concurrency,
            default_timeout_ms: timeout_ms,
            default_retry,
            ..RunOptions::default()
        };

        match engine
            .run_manifest_with_options(item_manifest, cwd, options)
            .await
        {
            Ok(_) => {
                checkpoint_statuses.insert(
                    format!("batch:{item_id}"),
                    StageStatus::Success(Value::Json(serde_json::json!({
                        "index": index,
                        "status": "succeeded"
                    }))),
                );
                if let Some(artifacts) = run_dir_artifacts {
                    write_batch_checkpoint(artifacts, &manifest_hash, &checkpoint_statuses)?;
                }
                write_batch_trace_event(
                    run_dir_artifacts,
                    serde_json::json!({
                        "run_id": format!("cli-batch-{item_id}"),
                        "event": "batch_item_finished",
                        "status": "success",
                        "timestamp_ms": timestamp_ms(),
                    }),
                )?;
                writeln!(
                    report,
                    "{}",
                    serde_json::json!({"index": index, "status": "succeeded"})
                )?;
            }
            Err(error) => {
                let kind = failure_kind(&error);
                let exit_code = llmff_exit_code(&error);
                let recommendation = retry_recommendation(&error);
                batch_failure.get_or_insert_with(|| BatchRunError {
                    report_path: report_path.clone(),
                    exit_code,
                    failure_kind: kind,
                    retry_recommendation: recommendation,
                });
                write_batch_trace_event(
                    run_dir_artifacts,
                    serde_json::json!({
                        "run_id": format!("cli-batch-{item_id}"),
                        "event": "batch_item_finished",
                        "status": "failed",
                        "timestamp_ms": timestamp_ms(),
                        "failure_kind": kind,
                        "failure_message": error.to_string(),
                    }),
                )?;
                writeln!(
                    report,
                    "{}",
                    serde_json::json!({
                        "index": index,
                        "status": "failed",
                        "exit_code": exit_code,
                        "failure_kind": kind,
                        "retry_recommendation": recommendation,
                        "message": error.to_string()
                    })
                )?;
            }
        }
    }
    report.flush()?;

    if let Some(error) = batch_failure {
        return Err(error.into());
    }

    Ok(())
}

#[derive(Debug)]
pub(super) struct BatchRunError {
    report_path: PathBuf,
    pub(super) exit_code: i32,
    pub(super) failure_kind: &'static str,
    pub(super) retry_recommendation: &'static str,
}

impl std::fmt::Display for BatchRunError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "one or more batch items failed; see {}",
            self.report_path.display()
        )
    }
}

impl std::error::Error for BatchRunError {}

pub fn batch_exit_code(error: &anyhow::Error) -> Option<i32> {
    error
        .chain()
        .find_map(|cause| cause.downcast_ref::<BatchRunError>())
        .map(|error| error.exit_code)
}
