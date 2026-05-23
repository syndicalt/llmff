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
| `run_failed` | The run failed before completion. The process still exits non-zero. |

Errors terminate the process with a non-zero exit. When an event writer has
been opened, `run_failed` is appended before returning the error. Supervisors
should still keep the process exit status as the final authority for run
failure because event output can be unavailable if the writer itself cannot be
created or flushed.

## Process Exit Codes

Supervisors should treat the process exit code as the final authority:

| Code | Meaning |
| --- | --- |
| `0` | Run or inspection completed successfully. |
| `1` | Unclassified internal failure. |
| `2` | Invalid CLI invocation or unsupported option combination. |
| `10` | Manifest, graph, configuration, or static validation failure before model/tool execution. |
| `20` | Stage execution failure or batch item failure. |
| `21` | Backend, provider, HTTP tool, or timeout failure. |
| `22` | Local I/O or JSON processing failure. |
| `30` | Selected behavior is intentionally not implemented. |

When a `run_failed` event is available, use `failure_kind` for the stable
machine-readable failure class and the exit code for the process outcome.

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
| `failure_kind` | string | `run_failed` | Stable failure class such as `manifest_parse`, `io`, `json`, `graph_validation`, `unknown_stage`, `stage_execution`, `backend`, `config`, or `not_implemented`. |
| `failure_message` | string | `run_failed` | Stable safe summary for the failure class. It does not include raw prompts, secrets, tool bodies, backend payloads, or provider response bodies. |

## Example Event

```json
{"run_id":"cli-run","event":"stage_finished","stage_id":"draft","op":"infer","status":"success","timestamp_ms":1780000000000,"duration_ms":12,"model":"mock:good","backend":"mock","provider_model":"good"}
```

## Example Failure Event

```json
{"run_id":"cli-run","event":"run_failed","status":"failed","timestamp_ms":1780000000010,"failure_kind":"backend","failure_message":"backend request failed"}
```

## Published Fixtures

Schema and JSONL fixtures are published for supervisor and dashboard tests:

- `examples/supervision/fixtures/event.schema.json`
- `examples/supervision/fixtures/success-trace.jsonl`
- `examples/supervision/fixtures/backend-error-trace.jsonl`

These fixtures cover the stable event envelope, stage timing, token usage,
cache hit metadata, and backend failure classification. Consumers should use
them as compatibility fixtures and continue to ignore unknown fields.

## Local Export Hooks

For post-run observability, write a trace and export it locally:

```bash
llmff run examples/json-repair.yaml --trace /tmp/llmff-trace.jsonl
scripts/trace-to-summary.sh /tmp/llmff-trace.jsonl
scripts/trace-to-metrics.sh /tmp/llmff-trace.jsonl
```

The exporters use only local files, Bash, and Python standard library modules.
They are safe hooks for later OpenTelemetry bridges because they do not start
collectors or send telemetry over the network.
`trace-to-summary.sh` reports stage timing, output and cache artifact
locations, token usage, cache hit rate, backend error rate, and sanitized
failure classifications from `run_failed`.

## Stream Separation

Only one live stream should own stdout. `llmff` rejects `--events -` together
with `--stream-stage` because both would write to stdout. It also rejects
`--events -` when manifest outputs write to `"-"`, and rejects `--stream-stage`
when manifest outputs write to `"-"`.

Use one of these layouts:

- Events on stdout, stage and final payloads written to files.
- Selected stage payload on stdout, events written to `--events <path>`.
- Final output on stdout, events written to `--events <path>`, with no
  `--stream-stage`.
