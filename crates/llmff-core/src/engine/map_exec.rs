use std::collections::BTreeMap;
use std::time::Instant;

use futures::StreamExt;

use crate::error::LlmffError;
use crate::manifest::StageSpec;
use crate::stage::get_json_path;
use crate::trace::{TraceEvent, TraceWriter};
use crate::value::{StageStatus, Value};

use super::streaming::StageStreamWriter;
use super::trace_failure::write_trace;
use super::{
    serialize_value_to_json, status_name, success_value, timestamp_ms, Engine, ExecutionContext,
    StageOutcome,
};

struct MapItemResult {
    index: usize,
    status: &'static str,
    value: serde_json::Value,
    trace_events: Vec<TraceEvent>,
}

struct MapItemExecution<'a, 'ctx> {
    context: &'a ExecutionContext<'ctx>,
    map_stage: &'a StageSpec,
    source: &'a Value,
    ordered_body: &'a [StageSpec],
    final_stage_id: &'a str,
    index: usize,
    item: serde_json::Value,
    stream_writer: Option<&'a mut StageStreamWriter>,
    trace_enabled: bool,
}

#[derive(Clone, Copy)]
struct MapTraceContext<'a> {
    run_id: &'a str,
    map_stage: &'a StageSpec,
    index: usize,
    body_stage: &'a StageSpec,
    trace_stage: &'a StageSpec,
}

impl Engine {
    pub(super) async fn execute_map(
        &self,
        context: &ExecutionContext<'_>,
        stage: &StageSpec,
        statuses: &BTreeMap<String, StageStatus>,
        mut stream_writer: Option<&mut StageStreamWriter>,
        trace: Option<&mut Vec<TraceWriter>>,
    ) -> Result<StageStatus, LlmffError> {
        let source = stage
            .from
            .as_ref()
            .and_then(|parent| statuses.get(parent))
            .and_then(success_value)
            .ok_or_else(|| LlmffError::StageExecution {
                stage_id: stage.id.clone(),
                message: "map requires successful input".to_string(),
            })?;
        let Value::Json(source_json) = source.clone() else {
            return Err(LlmffError::StageExecution {
                stage_id: stage.id.clone(),
                message: "map requires JSON input".to_string(),
            });
        };
        let items_path = stage
            .items_from
            .as_deref()
            .ok_or_else(|| LlmffError::StageExecution {
                stage_id: stage.id.clone(),
                message: "map requires items_from".to_string(),
            })?;
        let items_value =
            get_json_path(&source_json, items_path).ok_or_else(|| LlmffError::StageExecution {
                stage_id: stage.id.clone(),
                message: format!("map items_from path `{items_path}` was not found"),
            })?;
        let serde_json::Value::Array(items) = items_value else {
            return Err(LlmffError::StageExecution {
                stage_id: stage.id.clone(),
                message: format!("map items_from path `{items_path}` must resolve to an array"),
            });
        };
        let max_items = stage.max_items.unwrap_or(0);
        let item_count = items.len().min(max_items);
        let ordered_body = crate::graph::order_map_body_stages(stage)?;
        let final_stage_id = stage
            .final_output
            .as_ref()
            .map(|final_output| final_output.from.as_str())
            .or_else(|| stage.body.last().map(|body_stage| body_stage.id.as_str()))
            .ok_or_else(|| LlmffError::StageExecution {
                stage_id: stage.id.clone(),
                message: "map requires final stage".to_string(),
            })?;
        let trace_enabled = trace.is_some();
        let parallel = stage.parallel.unwrap_or(false);
        let item_inputs = items
            .iter()
            .take(item_count)
            .cloned()
            .enumerate()
            .collect::<Vec<_>>();
        let mut item_results = if parallel {
            let max_concurrency = stage.max_concurrency.unwrap_or(1).max(1);
            futures::stream::iter(item_inputs)
                .map(|(index, item)| {
                    self.execute_map_item(MapItemExecution {
                        context,
                        map_stage: stage,
                        source: &source,
                        ordered_body: &ordered_body,
                        final_stage_id,
                        index,
                        item,
                        stream_writer: None,
                        trace_enabled,
                    })
                })
                .buffer_unordered(max_concurrency)
                .collect::<Vec<_>>()
                .await
        } else {
            let mut results = Vec::with_capacity(item_count);
            for (index, item) in item_inputs {
                results.push(
                    self.execute_map_item(MapItemExecution {
                        context,
                        map_stage: stage,
                        source: &source,
                        ordered_body: &ordered_body,
                        final_stage_id,
                        index,
                        item,
                        stream_writer: stream_writer.as_deref_mut(),
                        trace_enabled,
                    })
                    .await,
                );
            }
            results
        }
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

        item_results.sort_by_key(|item| item.index);
        if let Some(trace) = trace {
            for item in &item_results {
                for event in &item.trace_events {
                    write_trace(trace, event.clone())?;
                }
            }
        }
        let mapped_items = item_results
            .into_iter()
            .map(|item| {
                serde_json::json!({
                    "index": item.index,
                    "status": item.status,
                    "value": item.value
                })
            })
            .collect::<Vec<_>>();

        let stop_reason = if items.len() > item_count {
            "max_items"
        } else {
            "completed"
        };

        Ok(StageStatus::Success(Value::Json(serde_json::json!({
            "items": mapped_items,
            "metadata": {
                "items_run": item_count,
                "items_total": items.len(),
                "stop_reason": stop_reason,
                "parallel": parallel
            }
        }))))
    }

    async fn execute_map_item(
        &self,
        mut item_context: MapItemExecution<'_, '_>,
    ) -> Result<MapItemResult, LlmffError> {
        let mut trace_events = Vec::new();
        let mut body_statuses = BTreeMap::new();
        body_statuses.insert(
            "input".to_string(),
            StageStatus::Success(item_context.source.clone()),
        );
        body_statuses.insert(
            "item".to_string(),
            StageStatus::Success(Value::Json(item_context.item)),
        );

        for body_stage in item_context.ordered_body {
            let trace_stage =
                map_trace_stage(item_context.map_stage, item_context.index, body_stage);
            let trace_context = MapTraceContext {
                run_id: item_context.context.run_id,
                map_stage: item_context.map_stage,
                index: item_context.index,
                body_stage,
                trace_stage: &trace_stage,
            };
            let stage_started = Instant::now();
            if item_context.trace_enabled {
                trace_events.push(self.map_trace_started(trace_context));
            }
            let outcome = match Box::pin(self.execute_stage_with_timeout(
                item_context.context,
                body_stage,
                &body_statuses,
                item_context.stream_writer.as_deref_mut(),
                None,
            ))
            .await
            {
                Ok(outcome) => outcome,
                Err(error) => {
                    if item_context.trace_enabled {
                        trace_events.push(self.map_trace_error(trace_context, stage_started));
                    }
                    return Err(error);
                }
            };
            if item_context.trace_enabled {
                trace_events.push(self.map_trace_finished(trace_context, stage_started, &outcome));
            }
            body_statuses.insert(body_stage.id.clone(), outcome.status);
        }

        let final_status = body_statuses
            .get(item_context.final_stage_id)
            .ok_or_else(|| LlmffError::StageExecution {
                stage_id: item_context.map_stage.id.clone(),
                message: format!(
                    "map final stage `{}` did not run",
                    item_context.final_stage_id
                ),
            })?;
        let (status, value) = map_item_output(
            item_context.map_stage,
            item_context.final_stage_id,
            final_status,
        )?;
        Ok(MapItemResult {
            index: item_context.index,
            status,
            value,
            trace_events,
        })
    }

    fn map_trace_started(&self, context: MapTraceContext<'_>) -> TraceEvent {
        TraceEvent {
            run_id: context.run_id.to_string(),
            event: "stage_started".to_string(),
            agent: context.body_stage.agent.clone(),
            stage_id: Some(context.trace_stage.id.clone()),
            loop_id: None,
            loop_iteration: None,
            loop_stage_id: None,
            map_id: Some(context.map_stage.id.clone()),
            map_index: Some(context.index),
            map_stage_id: Some(context.body_stage.id.clone()),
            op: Some(context.body_stage.op.clone()),
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
        }
    }

    fn map_trace_finished(
        &self,
        context: MapTraceContext<'_>,
        stage_started: Instant,
        outcome: &StageOutcome,
    ) -> TraceEvent {
        let status_name = status_name(&outcome.status).to_string();
        let metadata =
            self.trace_metadata(context.body_stage, &outcome.status, outcome.usage.as_ref());
        TraceEvent {
            run_id: context.run_id.to_string(),
            event: "stage_finished".to_string(),
            agent: context.body_stage.agent.clone(),
            stage_id: Some(context.trace_stage.id.clone()),
            loop_id: None,
            loop_iteration: None,
            loop_stage_id: None,
            map_id: Some(context.map_stage.id.clone()),
            map_index: Some(context.index),
            map_stage_id: Some(context.body_stage.id.clone()),
            op: Some(context.body_stage.op.clone()),
            status: Some(status_name),
            timestamp_ms: timestamp_ms(),
            duration_ms: Some(stage_started.elapsed().as_millis()),
            attempts: outcome.attempts,
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
            cache_hit: outcome.cache_hit,
            cache_path: outcome.cache_path.clone(),
            failure_kind: None,
            failure_message: None,
        }
    }

    fn map_trace_error(&self, context: MapTraceContext<'_>, stage_started: Instant) -> TraceEvent {
        TraceEvent {
            run_id: context.run_id.to_string(),
            event: "stage_finished".to_string(),
            agent: context.body_stage.agent.clone(),
            stage_id: Some(context.trace_stage.id.clone()),
            loop_id: None,
            loop_iteration: None,
            loop_stage_id: None,
            map_id: Some(context.map_stage.id.clone()),
            map_index: Some(context.index),
            map_stage_id: Some(context.body_stage.id.clone()),
            op: Some(context.body_stage.op.clone()),
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
        }
    }
}

fn map_trace_stage(map_stage: &StageSpec, index: usize, body_stage: &StageSpec) -> StageSpec {
    let mut trace_stage = body_stage.clone();
    trace_stage.id = format!("{}[{index}].{}", map_stage.id, body_stage.id);
    trace_stage
}

fn map_item_output(
    map_stage: &StageSpec,
    final_stage_id: &str,
    final_status: &StageStatus,
) -> Result<(&'static str, serde_json::Value), LlmffError> {
    let require_status = map_stage
        .final_output
        .as_ref()
        .and_then(|final_output| final_output.require_status.as_deref())
        .unwrap_or("success");
    match (require_status, final_status) {
        ("success", StageStatus::Success(value)) => {
            Ok(("success", serialize_value_to_json(value)?))
        }
        ("invalid", StageStatus::Invalid { value, .. }) => {
            Ok(("invalid", serialize_value_to_json(value)?))
        }
        ("any", StageStatus::Success(value)) => Ok(("success", serialize_value_to_json(value)?)),
        ("any", StageStatus::Invalid { value, .. }) => {
            Ok(("invalid", serialize_value_to_json(value)?))
        }
        ("any", StageStatus::Skipped) => Ok(("skipped", serde_json::Value::Null)),
        ("success", StageStatus::Invalid { errors, .. }) => Err(LlmffError::StageExecution {
            stage_id: map_stage.id.clone(),
            message: format!(
                "map final stage `{final_stage_id}` was invalid: {}",
                errors.join("; ")
            ),
        }),
        ("success", StageStatus::Skipped) => Err(LlmffError::StageExecution {
            stage_id: map_stage.id.clone(),
            message: format!("map final stage `{final_stage_id}` was skipped"),
        }),
        ("invalid", StageStatus::Success(_)) | ("invalid", StageStatus::Skipped) => {
            Err(LlmffError::StageExecution {
                stage_id: map_stage.id.clone(),
                message: format!("map final stage `{final_stage_id}` did not finish invalid"),
            })
        }
        _ => Err(LlmffError::StageExecution {
            stage_id: map_stage.id.clone(),
            message: "final.require_status must be success, invalid, or any".to_string(),
        }),
    }
}
