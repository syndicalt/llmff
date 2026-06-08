use std::collections::BTreeMap;

use crate::error::LlmffError;
use crate::graph::stage_dependencies;
use crate::trace::TraceWriter;
use crate::value::StageStatus;

use super::checkpoint::write_checkpoint_if_configured;
use super::streaming::{stream_stage_payload_if_selected, StageStreamWriter};
use super::{Engine, ExecutionContext};

pub(super) async fn run_stages_sequentially(
    engine: &Engine,
    context: &ExecutionContext<'_>,
    statuses: &mut BTreeMap<String, StageStatus>,
    trace: &mut Vec<TraceWriter>,
    mut stream_writer: Option<&mut StageStreamWriter>,
) -> Result<(), LlmffError> {
    for stage in context.graph.stages() {
        if statuses.contains_key(&stage.id) {
            continue;
        }
        let stage_started = engine.start_stage_trace(trace, context.run_id, stage)?;
        let outcome = engine
            .execute_stage_with_timeout(
                context,
                stage,
                statuses,
                stream_writer.as_deref_mut(),
                Some(&mut *trace),
            )
            .await?;
        stream_stage_payload_if_selected(stream_writer.as_deref_mut(), stage, &outcome)?;
        engine.finish_stage_trace(
            trace,
            context.run_id,
            stage,
            stage_started,
            outcome,
            statuses,
        )?;
        write_checkpoint_if_configured(
            context.options.checkpoint_path.as_deref(),
            statuses,
            context.manifest_hash,
        )?;
    }

    Ok(())
}

pub(super) async fn run_stages_in_parallel(
    engine: &Engine,
    context: &ExecutionContext<'_>,
    statuses: &mut BTreeMap<String, StageStatus>,
    trace: &mut Vec<TraceWriter>,
) -> Result<(), LlmffError> {
    let mut pending = context.graph.stages().iter().collect::<Vec<_>>();

    while !pending.is_empty() {
        let mut ready = Vec::new();
        let mut waiting = Vec::new();

        for stage in pending {
            if statuses.contains_key(&stage.id) {
                continue;
            }
            if stage_dependencies(stage)
                .iter()
                .all(|dependency| statuses.contains_key(dependency))
            {
                ready.push(stage);
            } else {
                waiting.push(stage);
            }
        }

        if ready.is_empty() && waiting.is_empty() {
            break;
        }
        if ready.is_empty() {
            return Err(LlmffError::GraphValidation(
                "cycle detected in graph".to_string(),
            ));
        }

        let max_concurrency = context
            .options
            .max_concurrency
            .unwrap_or(ready.len())
            .max(1);
        for chunk in ready.chunks(max_concurrency) {
            let starts = chunk
                .iter()
                .map(|stage| engine.start_stage_trace(trace, context.run_id, stage))
                .collect::<Result<Vec<_>, _>>()?;
            let status_snapshot = statuses.clone();
            let status_snapshot_ref = &status_snapshot;
            let mut outcomes = (0..chunk.len()).map(|_| None).collect::<Vec<_>>();
            let non_loop_outcomes = futures::future::join_all(
                chunk
                    .iter()
                    .enumerate()
                    .filter(|(_, stage)| stage.op != "loop")
                    .map(|(index, stage)| async move {
                        (
                            index,
                            engine
                                .execute_stage_with_timeout(
                                    context,
                                    stage,
                                    status_snapshot_ref,
                                    None,
                                    None,
                                )
                                .await,
                        )
                    }),
            )
            .await;
            for (index, outcome) in non_loop_outcomes {
                outcomes[index] = Some(outcome);
            }
            for (index, stage) in chunk
                .iter()
                .enumerate()
                .filter(|(_, stage)| stage.op == "loop")
            {
                outcomes[index] = Some(
                    engine
                        .execute_stage_with_timeout(
                            context,
                            stage,
                            &status_snapshot,
                            None,
                            Some(&mut *trace),
                        )
                        .await,
                );
            }

            for ((stage, started), outcome) in chunk.iter().zip(starts).zip(outcomes) {
                engine.finish_stage_trace(
                    trace,
                    context.run_id,
                    stage,
                    started,
                    outcome.expect("parallel stage outcome should be recorded")?,
                    statuses,
                )?;
                write_checkpoint_if_configured(
                    context.options.checkpoint_path.as_deref(),
                    statuses,
                    context.manifest_hash,
                )?;
            }
        }

        pending = waiting;
    }

    Ok(())
}
