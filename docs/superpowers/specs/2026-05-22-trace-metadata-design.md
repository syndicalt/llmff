# Trace Metadata Design

## Goal

Make trace JSONL useful for debugging and replay inspection by adding per-stage timing and operation metadata.

## Scope

This is an additive trace schema update. Existing fields remain:

- `run_id`
- `event`
- `stage_id`
- `op`
- `status`

New optional fields:

- `timestamp_ms`: Unix epoch milliseconds for every trace event.
- `duration_ms`: elapsed milliseconds for `stage_finished` events.
- `model`: manifest model id for model-calling stages.
- `backend`: backend alias selected for model-calling stages.
- `provider_model`: provider model id sent to the backend.
- `validation_errors`: validation errors for invalid stage outputs.
- `tool_kind`: `command` or `http` for tool stages.
- `tool_target`: executable path or URL for tool stages.
- `output_path`: path for `write` stages.

## Semantics

- Timestamps are generated at write time in the engine.
- Stage duration is measured around `execute_stage`.
- Model metadata is derived from the manifest model id and backend alias resolution, not from provider response text.
- Validation errors are included only when the stage status is `Invalid`.
- Tool command metadata must not include stdin, stdout, stderr, or secrets. It only records the executable path.
- HTTP tool metadata records the URL configured in the manifest. Headers and body are not traced.
- Write metadata records configured `path`.

## Non-Goals

- No full input/output payload tracing in this slice.
- No token usage accounting yet.
- No trace redaction engine yet.
- No trace command/viewer yet.

## Tests

- Trace events include `timestamp_ms` on run and stage events.
- `stage_finished` includes `duration_ms`.
- Inference traces include `model`, `backend`, and `provider_model`.
- Invalid validation traces include `validation_errors`.
- Tool and write traces include safe operation metadata.
