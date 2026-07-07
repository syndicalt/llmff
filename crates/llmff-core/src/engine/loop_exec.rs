use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use crate::error::LlmffError;
use crate::manifest::{LoopBreakSpec, LoopRetentionSpec, StageSpec};
use crate::trace::{TraceEvent, TraceWriter};
use crate::value::{StageStatus, Value};

use super::streaming::StageStreamWriter;
use super::trace_failure::write_trace;
use super::{
    serialize_value_to_json, stage_validation_error, status_name, success_value, timestamp_ms,
    Engine, ExecutionContext, StageOutcome,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LoopTraceContext<'a> {
    loop_id: &'a str,
    iteration: usize,
}

struct FinishStageTrace<'a> {
    trace_stage: &'a StageSpec,
    status_stage_id: &'a str,
    loop_stage_id: &'a str,
    stage_started: Instant,
    outcome: StageOutcome,
    loop_context: Option<LoopTraceContext<'a>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
enum LoopStopReason {
    BreakCondition,
    MaxIterations,
    Error,
}

pub(super) struct LoopRetentionConfig {
    mode: String,
    stages: BTreeSet<String>,
    include_values: bool,
}

impl Engine {
    pub(super) fn start_stage_trace(
        &self,
        trace: &mut Vec<TraceWriter>,
        run_id: &str,
        stage: &StageSpec,
    ) -> Result<Instant, LlmffError> {
        self.start_stage_trace_with_loop(trace, run_id, stage, &stage.id, None)
    }

    fn start_stage_trace_with_loop(
        &self,
        trace: &mut Vec<TraceWriter>,
        run_id: &str,
        stage: &StageSpec,
        loop_stage_id: &str,
        loop_context: Option<LoopTraceContext<'_>>,
    ) -> Result<Instant, LlmffError> {
        let started = Instant::now();
        let loop_id = loop_context.map(|context| context.loop_id.to_string());
        let loop_iteration = loop_context.map(|context| context.iteration);
        write_trace(
            trace,
            TraceEvent {
                run_id: run_id.to_string(),
                event: "stage_started".to_string(),
                agent: stage.agent.clone(),
                stage_id: Some(stage.id.clone()),
                loop_id,
                loop_iteration,
                loop_stage_id: loop_context.map(|_| loop_stage_id.to_string()),
                map_id: None,
                map_index: None,
                map_stage_id: None,
                op: Some(stage.op.clone()),
                status: None,
                timestamp_ms: timestamp_ms(),
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
                failure_kind: None,
                failure_message: None,
            },
        )?;
        Ok(started)
    }

    pub(super) fn finish_stage_trace(
        &self,
        trace: &mut Vec<TraceWriter>,
        run_id: &str,
        stage: &StageSpec,
        stage_started: Instant,
        outcome: StageOutcome,
        statuses: &mut BTreeMap<String, StageStatus>,
    ) -> Result<(), LlmffError> {
        self.finish_stage_trace_with_loop(
            trace,
            run_id,
            statuses,
            FinishStageTrace {
                trace_stage: stage,
                status_stage_id: &stage.id,
                loop_stage_id: &stage.id,
                stage_started,
                outcome,
                loop_context: None,
            },
        )
    }

    fn finish_stage_trace_with_loop(
        &self,
        trace: &mut Vec<TraceWriter>,
        run_id: &str,
        statuses: &mut BTreeMap<String, StageStatus>,
        finish: FinishStageTrace<'_>,
    ) -> Result<(), LlmffError> {
        let trace_stage = finish.trace_stage;
        let outcome = finish.outcome;
        let status = outcome.status;
        let status_name = status_name(&status).to_string();
        let metadata = self.trace_metadata(trace_stage, &status, outcome.usage.as_ref());
        let cache_hit = outcome.cache_hit;
        let cache_path = outcome.cache_path;
        let attempts = outcome.attempts;
        let loop_id = finish
            .loop_context
            .map(|context| context.loop_id.to_string());
        let loop_iteration = finish.loop_context.map(|context| context.iteration);
        statuses.insert(finish.status_stage_id.to_string(), status);
        write_trace(
            trace,
            TraceEvent {
                run_id: run_id.to_string(),
                event: "stage_finished".to_string(),
                agent: trace_stage.agent.clone(),
                stage_id: Some(trace_stage.id.clone()),
                loop_id,
                loop_iteration,
                loop_stage_id: finish
                    .loop_context
                    .map(|_| finish.loop_stage_id.to_string()),
                map_id: None,
                map_index: None,
                map_stage_id: None,
                op: Some(trace_stage.op.clone()),
                status: Some(status_name),
                timestamp_ms: timestamp_ms(),
                duration_ms: Some(finish.stage_started.elapsed().as_millis()),
                attempts,
                model: metadata.model,
                backend: metadata.backend,
                provider_model: metadata.provider_model,
                validation_errors: metadata.validation_errors,
                tool_kind: metadata.tool_kind,
                tool_target: metadata.tool_target,
                output_path: metadata.output_path,
                prompt_tokens: metadata.prompt_tokens,
                completion_tokens: metadata.completion_tokens,
                total_tokens: metadata.total_tokens,
                cache_hit,
                cache_path,
                failure_kind: None,
                failure_message: None,
            },
        )
    }

    fn finish_stage_trace_error_with_loop(
        &self,
        trace: &mut Vec<TraceWriter>,
        run_id: &str,
        trace_stage: &StageSpec,
        loop_stage_id: &str,
        stage_started: Instant,
        loop_context: LoopTraceContext<'_>,
    ) -> Result<(), LlmffError> {
        write_trace(
            trace,
            TraceEvent {
                run_id: run_id.to_string(),
                event: "stage_finished".to_string(),
                agent: trace_stage.agent.clone(),
                stage_id: Some(trace_stage.id.clone()),
                loop_id: Some(loop_context.loop_id.to_string()),
                loop_iteration: Some(loop_context.iteration),
                loop_stage_id: Some(loop_stage_id.to_string()),
                map_id: None,
                map_index: None,
                map_stage_id: None,
                op: Some(trace_stage.op.clone()),
                status: Some("error".to_string()),
                timestamp_ms: timestamp_ms(),
                duration_ms: Some(stage_started.elapsed().as_millis()),
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
                failure_kind: None,
                failure_message: None,
            },
        )
    }

    pub(super) async fn execute_loop(
        &self,
        context: &ExecutionContext<'_>,
        stage: &StageSpec,
        statuses: &BTreeMap<String, StageStatus>,
        mut stream_writer: Option<&mut StageStreamWriter>,
        mut trace: Option<&mut Vec<TraceWriter>>,
    ) -> Result<StageStatus, LlmffError> {
        let source = stage
            .from
            .as_ref()
            .and_then(|parent| statuses.get(parent))
            .and_then(success_value)
            .ok_or_else(|| LlmffError::StageExecution {
                stage_id: stage.id.clone(),
                message: "loop requires successful input".to_string(),
            })?;
        let max_iterations = stage.max_iterations.unwrap_or(0);
        let ordered_body = crate::graph::order_loop_body_stages(stage)?;
        let final_stage_id = stage
            .final_output
            .as_ref()
            .map(|final_output| final_output.from.as_str())
            .or_else(|| stage.body.last().map(|body_stage| body_stage.id.as_str()))
            .ok_or_else(|| LlmffError::StageExecution {
                stage_id: stage.id.clone(),
                message: "loop requires final stage".to_string(),
            })?;
        let iteration_error_policy = stage.on_iteration_error.as_deref().unwrap_or("fail");
        let retention = loop_retention_config(stage)?;
        let mut retained_iterations = Vec::new();
        let mut previous_body_statuses: BTreeMap<String, StageStatus> = BTreeMap::new();
        let mut last_body_statuses: BTreeMap<String, StageStatus> = BTreeMap::new();
        let mut latest_final_status = None;
        let mut latest_iteration_error = None;
        let mut stop_reason = LoopStopReason::MaxIterations;
        let mut iterations_run = 0usize;

        for iteration in 1..=max_iterations {
            iterations_run = iteration;
            let mut body_statuses = BTreeMap::new();
            body_statuses.insert("input".to_string(), StageStatus::Success(source.clone()));
            if iteration == 1 {
                for (input_name, value) in &stage.initial_carry {
                    body_statuses.insert(
                        input_name.clone(),
                        StageStatus::Success(Value::Json(value.clone())),
                    );
                }
            }
            for (input_name, previous_stage_id) in &stage.carry {
                if let Some(previous_status) = previous_body_statuses.get(previous_stage_id) {
                    body_statuses.insert(input_name.clone(), previous_status.clone());
                }
            }

            for body_stage in &ordered_body {
                if iteration == 1 {
                    reject_missing_first_iteration_carry_alias(stage, body_stage, &body_statuses)?;
                }

                let trace_stage = loop_trace_stage(stage, body_stage);
                let loop_context = LoopTraceContext {
                    loop_id: &stage.id,
                    iteration,
                };
                let stage_started = if let Some(trace) = trace.as_deref_mut() {
                    Some(self.start_stage_trace_with_loop(
                        trace,
                        context.run_id,
                        &trace_stage,
                        &body_stage.id,
                        Some(loop_context),
                    )?)
                } else {
                    None
                };
                let outcome = match Box::pin(self.execute_stage_with_timeout(
                    context,
                    body_stage,
                    &body_statuses,
                    stream_writer.as_deref_mut(),
                    None,
                ))
                .await
                {
                    Ok(outcome) => outcome,
                    Err(error) => match iteration_error_policy {
                        "fail" => {
                            return Err(loop_trace_error(
                                error,
                                &trace_stage,
                                &body_stage.id,
                                loop_context,
                            ));
                        }
                        "break" => {
                            if let (Some(trace), Some(stage_started)) =
                                (trace.as_deref_mut(), stage_started)
                            {
                                self.finish_stage_trace_error_with_loop(
                                    trace,
                                    context.run_id,
                                    &trace_stage,
                                    &body_stage.id,
                                    stage_started,
                                    loop_context,
                                )?;
                            }
                            latest_iteration_error = Some((iteration, body_stage.id.clone()));
                            stop_reason = LoopStopReason::Error;
                            break;
                        }
                        "continue" => {
                            if let (Some(trace), Some(stage_started)) =
                                (trace.as_deref_mut(), stage_started)
                            {
                                self.finish_stage_trace_error_with_loop(
                                    trace,
                                    context.run_id,
                                    &trace_stage,
                                    &body_stage.id,
                                    stage_started,
                                    loop_context,
                                )?;
                            }
                            latest_iteration_error = Some((iteration, body_stage.id.clone()));
                            break;
                        }
                        _ => return Err(error),
                    },
                };
                if let (Some(trace), Some(stage_started)) = (trace.as_deref_mut(), stage_started) {
                    self.finish_stage_trace_with_loop(
                        trace,
                        context.run_id,
                        &mut body_statuses,
                        FinishStageTrace {
                            trace_stage: &trace_stage,
                            status_stage_id: &body_stage.id,
                            loop_stage_id: &body_stage.id,
                            stage_started,
                            outcome,
                            loop_context: Some(loop_context),
                        },
                    )?;
                } else {
                    body_statuses.insert(body_stage.id.clone(), outcome.status);
                }
                if body_stage.id == final_stage_id {
                    if let Some(final_status) = body_statuses
                        .get(final_stage_id)
                        .filter(|status| !matches!(status, StageStatus::Skipped))
                    {
                        latest_final_status = Some(final_status.clone());
                    }
                }
            }

            if let Some(retention) = &retention {
                retained_iterations.push(loop_iteration_record(
                    iteration,
                    &ordered_body,
                    &body_statuses,
                    retention,
                )?);
            }

            if matches!(stop_reason, LoopStopReason::Error) {
                if body_statuses.contains_key(final_stage_id) {
                    last_body_statuses = body_statuses;
                }
                break;
            }

            if latest_iteration_error
                .as_ref()
                .is_some_and(|(error_iteration, _)| *error_iteration == iteration)
            {
                previous_body_statuses = body_statuses;
                continue;
            }

            let should_break =
                evaluate_loop_break(stage.break_on.as_ref().unwrap(), &body_statuses)?;
            previous_body_statuses = body_statuses.clone();
            last_body_statuses = body_statuses;
            if should_break {
                stop_reason = LoopStopReason::BreakCondition;
                break;
            }
        }

        let final_status = last_body_statuses
            .get(final_stage_id)
            .filter(|status| !matches!(status, StageStatus::Skipped))
            .or(latest_final_status.as_ref())
            .ok_or_else(|| {
                if let Some((iteration, body_stage_id)) = latest_iteration_error.as_ref() {
                    let message = if matches!(stop_reason, LoopStopReason::Error) {
                        format!(
                            "loop `{}` stopped on iteration error at iteration {iteration} in body stage `{body_stage_id}` with no final output available",
                            stage.id
                        )
                    } else {
                        format!(
                            "loop `{}` produced no final output after iteration errors",
                            stage.id
                        )
                    };
                    return LlmffError::StageExecution {
                        stage_id: stage.id.clone(),
                        message,
                    };
                }

                LlmffError::StageExecution {
                    stage_id: stage.id.clone(),
                    message: format!("loop final stage `{final_stage_id}` did not run"),
                }
            })?;
        let final_value = loop_final_value(
            stage,
            final_stage_id,
            final_status,
            iterations_run,
            &stop_reason,
            &retained_iterations,
        )?;

        Ok(StageStatus::Success(final_value))
    }
}

fn loop_trace_stage(loop_stage: &StageSpec, body_stage: &StageSpec) -> StageSpec {
    let mut trace_stage = body_stage.clone();
    trace_stage.id = format!("{}.{}", loop_stage.id, body_stage.id);
    trace_stage
}

fn loop_trace_error(
    error: LlmffError,
    trace_stage: &StageSpec,
    loop_stage_id: &str,
    loop_context: LoopTraceContext<'_>,
) -> LlmffError {
    LlmffError::LoopStageExecution {
        stage_id: trace_stage.id.clone(),
        loop_id: loop_context.loop_id.to_string(),
        loop_iteration: loop_context.iteration,
        loop_stage_id: loop_stage_id.to_string(),
        source: Box::new(error),
    }
}

pub(super) fn loop_retention_config(
    stage: &StageSpec,
) -> Result<Option<LoopRetentionConfig>, LlmffError> {
    let Some(retention) = &stage.retain_iterations else {
        return Ok(None);
    };
    let (mode, stages, include_values) = match retention {
        LoopRetentionSpec::Mode(mode) => (
            mode.clone(),
            BTreeSet::new(),
            matches!(mode.as_str(), "all"),
        ),
        LoopRetentionSpec::Config {
            mode,
            stages,
            include_values,
        } => (
            mode.clone(),
            stages.iter().cloned().collect::<BTreeSet<_>>(),
            include_values.unwrap_or_else(|| mode == "all"),
        ),
    };
    if mode == "none" {
        return Ok(None);
    }
    if !matches!(mode.as_str(), "summaries" | "all") {
        return Err(stage_validation_error(
            stage,
            "retain_iterations must be none, summaries, or all",
        ));
    }

    Ok(Some(LoopRetentionConfig {
        mode,
        stages,
        include_values,
    }))
}

fn loop_iteration_record(
    iteration: usize,
    ordered_body: &[StageSpec],
    body_statuses: &BTreeMap<String, StageStatus>,
    retention: &LoopRetentionConfig,
) -> Result<serde_json::Value, LlmffError> {
    let mut stages = serde_json::Map::new();
    for body_stage in ordered_body {
        if !retention.stages.is_empty() && !retention.stages.contains(&body_stage.id) {
            continue;
        }
        let Some(status) = body_statuses.get(&body_stage.id) else {
            continue;
        };
        let mut stage_record = serde_json::Map::new();
        stage_record.insert(
            "status".to_string(),
            serde_json::Value::String(status_name(status).to_string()),
        );
        if retention.include_values {
            match status {
                StageStatus::Success(value) => {
                    stage_record.insert("value".to_string(), serialize_value_to_json(value)?);
                }
                StageStatus::Invalid { value, errors } => {
                    stage_record.insert("value".to_string(), serialize_value_to_json(value)?);
                    stage_record.insert(
                        "errors".to_string(),
                        serde_json::Value::Array(
                            errors
                                .iter()
                                .cloned()
                                .map(serde_json::Value::String)
                                .collect(),
                        ),
                    );
                }
                StageStatus::Skipped => {}
            }
        }
        stages.insert(
            body_stage.id.clone(),
            serde_json::Value::Object(stage_record),
        );
    }

    Ok(serde_json::json!({
        "iteration": iteration,
        "mode": retention.mode.clone(),
        "stages": stages
    }))
}

fn evaluate_loop_break(
    break_on: &LoopBreakSpec,
    statuses: &BTreeMap<String, StageStatus>,
) -> Result<bool, LlmffError> {
    match break_on {
        LoopBreakSpec::StageSuccess { stage } => {
            Ok(matches!(statuses.get(stage), Some(StageStatus::Success(_))))
        }
        LoopBreakSpec::StageFailure { stage } => Ok(matches!(
            statuses.get(stage),
            Some(StageStatus::Invalid { .. }) | Some(StageStatus::Skipped)
        )),
        LoopBreakSpec::FieldTrue { stage, field } => {
            let Some(value) = statuses.get(stage).and_then(success_value) else {
                return Ok(false);
            };
            Ok(json_field(&value, field)
                .and_then(|value| value.as_bool())
                .unwrap_or(false))
        }
        LoopBreakSpec::FieldEquals {
            stage,
            field,
            value,
        } => {
            let Some(actual) = statuses
                .get(stage)
                .and_then(success_value)
                .and_then(|value| json_field(&value, field).cloned())
            else {
                return Ok(false);
            };
            Ok(actual == *value)
        }
        LoopBreakSpec::Never => Ok(false),
    }
}

fn json_field<'a>(value: &'a Value, field: &str) -> Option<&'a serde_json::Value> {
    match value {
        Value::Json(json) => json.get(field),
        _ => None,
    }
}

fn reject_missing_first_iteration_carry_alias(
    loop_stage: &StageSpec,
    body_stage: &StageSpec,
    body_statuses: &BTreeMap<String, StageStatus>,
) -> Result<(), LlmffError> {
    for parent in [body_stage.from.as_ref(), body_stage.state_from.as_ref()]
        .into_iter()
        .flatten()
    {
        if !loop_stage.carry.contains_key(parent) || body_statuses.contains_key(parent) {
            continue;
        }

        return Err(LlmffError::StageExecution {
            stage_id: loop_stage.id.clone(),
            message: format!(
                "loop `{}` cannot read carry alias `{parent}` on iteration 1 before it has a previous value",
                loop_stage.id
            ),
        });
    }

    Ok(())
}

fn loop_final_value(
    loop_stage: &StageSpec,
    final_stage_id: &str,
    final_status: &StageStatus,
    iterations_run: usize,
    stop_reason: &LoopStopReason,
    retained_iterations: &[serde_json::Value],
) -> Result<Value, LlmffError> {
    let require_status = loop_stage
        .final_output
        .as_ref()
        .and_then(|final_output| final_output.require_status.as_deref())
        .unwrap_or("success");
    let final_payload = match (require_status, final_status) {
        ("success", StageStatus::Success(value)) => serialize_value_to_json(value)?,
        ("invalid", StageStatus::Invalid { value, .. }) => serialize_value_to_json(value)?,
        ("any", StageStatus::Success(value)) => serialize_value_to_json(value)?,
        ("any", StageStatus::Invalid { value, .. }) => serialize_value_to_json(value)?,
        ("any", StageStatus::Skipped) => serde_json::Value::Null,
        ("success", StageStatus::Invalid { errors, .. }) => {
            return Err(LlmffError::StageExecution {
                stage_id: loop_stage.id.clone(),
                message: format!(
                    "loop final stage `{final_stage_id}` was invalid: {}",
                    errors.join("; ")
                ),
            });
        }
        ("success", StageStatus::Skipped) => {
            return Err(LlmffError::StageExecution {
                stage_id: loop_stage.id.clone(),
                message: format!("loop final stage `{final_stage_id}` was skipped"),
            });
        }
        ("invalid", StageStatus::Success(_)) | ("invalid", StageStatus::Skipped) => {
            return Err(LlmffError::StageExecution {
                stage_id: loop_stage.id.clone(),
                message: format!("loop final stage `{final_stage_id}` did not finish invalid"),
            });
        }
        _ => serde_json::Value::Null,
    };

    let mut output = serde_json::Map::new();
    output.insert("final".to_string(), final_payload);
    output.insert(
        "metadata".to_string(),
        serde_json::json!({
            "iterations_run": iterations_run,
            "stop_reason": loop_stop_reason_name(stop_reason),
            "final_stage": final_stage_id
        }),
    );
    if !retained_iterations.is_empty() {
        output.insert(
            "iterations".to_string(),
            serde_json::Value::Array(retained_iterations.to_vec()),
        );
    }

    Ok(Value::Json(serde_json::Value::Object(output)))
}

fn loop_stop_reason_name(reason: &LoopStopReason) -> &'static str {
    match reason {
        LoopStopReason::BreakCondition => "break_condition",
        LoopStopReason::MaxIterations => "max_iterations",
        LoopStopReason::Error => "error",
    }
}
