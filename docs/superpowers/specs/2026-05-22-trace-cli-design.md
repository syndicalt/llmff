# Trace CLI Design

## Goal

Add `llmff trace <path>` so users can inspect trace JSONL without manually reading raw JSON lines.

## CLI Shape

```bash
llmff trace /tmp/llmff-trace.jsonl
```

Output is a compact text summary:

```text
run local-run succeeded
load_prompt load success 0ms
draft infer success 14ms model=openai:gpt-4.1-mini backend=openai provider_model=gpt-4.1-mini
validate validate_json invalid 1ms validation_errors=1
```

## Semantics

- Read a JSONL trace file from a path.
- Parse each line as JSON.
- Print one line per `run_finished` and `stage_finished` event.
- Preserve trace order.
- For `run_finished`: print `run <run_id> <status>`.
- For `stage_finished`: print `<stage_id> <op> <status> <duration_ms>ms` plus safe metadata if present:
  - `model`
  - `backend`
  - `provider_model`
  - `validation_errors=<count>`
  - `tool_kind`
  - `tool_target`
  - `output_path`
- Do not print prompts, response bodies, API keys, headers, tool stdin/stdout, or raw validation error contents.
- Invalid JSONL returns an error naming the line number.

## Non-Goals

- No TUI or pager.
- No JSON output mode yet.
- No filtering flags yet.
- No trace directory discovery yet; the command takes an explicit path in this slice.

## Tests

- CLI trace summary prints run status and stage metadata from a hand-written JSONL trace.
- CLI trace summary reports invalid JSONL line numbers.
