# Lifecycle Event Schema

`llmff run --events <path>` writes lifecycle events as newline-delimited JSON.
`llmff run --events -` writes the same event stream to stdout. The event stream
uses the trace event schema, but it is intended for live supervisors,
dashboards, and shell pipelines rather than post-run summaries.

## Compatibility Contract

Supervisors should treat events as an append-only JSONL protocol:

- Every non-empty line is one complete JSON object.
- `run_id`, `event`, and `timestamp_ms` are required on every event.
- Consumers must ignore unknown fields.
- Optional fields may be omitted when they do not apply to the event or stage.
- Event names and existing field meanings are stable within a major version.
- New event names, optional metadata fields, or backend-specific metadata may be
  added in minor versions.
- Event order is causal for a single run: `run_started` appears before stage
  events, and `run_finished` appears after all completed stage events.
- In parallel execution, stage events from independent stages may be interleaved.
  Supervisors should correlate by `run_id` and `stage_id`, not by adjacency.

## Event Types

| Event | Meaning |
| --- | --- |
| `run_started` | The manifest was accepted and execution is starting. |
| `stage_started` | A stage is about to execute. |
| `stage_finished` | A stage produced a success, invalid, or skipped status. |
| `run_finished` | The run completed successfully and outputs were written. |

Errors terminate the process with a non-zero exit. A failed process may leave a
partial event file; supervisors should keep the process exit status as the final
authority for run failure until a dedicated failure event is added.

## Fields

| Field | Type | Events | Description |
| --- | --- | --- | --- |
| `run_id` | string | all | Stable identifier for this CLI run. The CLI currently emits `cli-run`. |
| `event` | string | all | Event type. |
| `timestamp_ms` | integer | all | Unix timestamp in milliseconds when the event was emitted. |
| `stage_id` | string | stage events | Manifest stage id. |
| `op` | string | stage events | Stage operation such as `load`, `infer`, `retrieve`, or `write`. |
| `status` | string | `stage_finished`, `run_finished` | Stage or run status. Stage values include `success`, `invalid`, and `skipped`. |
| `duration_ms` | integer | `stage_finished` | Stage wall-clock duration in milliseconds. |
| `model` | string | model stages | Manifest model alias, when applicable. |
| `backend` | string | model stages | Resolved backend alias. |
| `provider_model` | string | model stages | Model name sent to the provider after alias resolution. |
| `prompt_tokens` | integer | model stages | Provider usage metadata, when available. |
| `completion_tokens` | integer | model stages | Provider usage metadata, when available. |
| `total_tokens` | integer | model stages | Provider usage metadata, when available. |
| `validation_errors` | array of strings | validation stages | JSON validation errors for invalid values. |
| `tool_kind` | string | tool stages | Tool transport kind. |
| `tool_target` | string | tool stages | Tool command, URL, or plugin transport name. |
| `output_path` | string | write stages | Destination path for a write stage. |
| `cache_hit` | boolean | cache stages | Whether the cache stage reused an existing value. |
| `cache_path` | string | cache stages | Cache file path used by the stage. |

## Example Event

```json
{"run_id":"cli-run","event":"stage_finished","stage_id":"draft","op":"infer","status":"success","timestamp_ms":1780000000000,"duration_ms":12,"model":"mock:good","backend":"mock","provider_model":"good"}
```

## Stream Separation

Only one live stream should own stdout. `llmff` rejects `--events -` together
with `--stream-stage` because both would write to stdout. It also rejects
`--stream-stage` when manifest outputs write to `"-"`.

Use one of these layouts:

- Events on stdout, stage and final payloads written to files.
- Selected stage payload on stdout, events written to `--events <path>`.
- Final output on stdout, events written to `--events <path>`, with no
  `--stream-stage`.
