# Usage Metadata Design

## Goal

Capture provider token usage when available and expose it in trace JSONL and `llmff trace` summaries.

## User Model

When a backend returns token usage, the `stage_finished` trace for a model-calling stage includes:

```json
{
  "event": "stage_finished",
  "stage_id": "draft",
  "op": "infer",
  "status": "success",
  "prompt_tokens": 12,
  "completion_tokens": 8,
  "total_tokens": 20
}
```

`llmff trace` prints compact metadata:

```text
draft infer success 14ms model=openai:gpt-test usage=20 prompt_tokens=12 completion_tokens=8
```

Usage fields are optional. Backends that do not provide usage keep traces unchanged.

## Backend Semantics

- `InferResponse` carries optional `UsageMetadata`.
- OpenAI-compatible backends parse `usage.prompt_tokens`, `usage.completion_tokens`, and `usage.total_tokens` when present.
- Ollama chat responses parse `prompt_eval_count` and `eval_count` when present:
  - `prompt_tokens = prompt_eval_count`
  - `completion_tokens = eval_count`
  - `total_tokens = prompt_eval_count + eval_count`
- Mock backends return no usage by default.

## Engine Semantics

The engine stores usage for successful model-calling stages and adds it to the matching `stage_finished` trace event. The usage is associated with the stage response, not guessed from prompt text or output text.

`repair` receives the same behavior as `infer`: if its backend response includes usage, its trace event includes usage.

## Scope

Included:

- Optional usage fields in backend responses.
- OpenAI-compatible and Ollama usage parsing.
- Trace JSONL fields for usage.
- Trace CLI summary display.
- README documentation and focused tests.

Excluded:

- Local tokenization or estimated token counts.
- Cost calculation.
- Aggregated run-level usage totals.
- Streaming token accounting.

## Acceptance Criteria

- OpenAI-compatible backend tests prove usage is parsed from response JSON.
- Ollama backend tests prove usage is parsed and total is computed.
- Trace event tests prove model stage traces include usage metadata when provided.
- `llmff trace` summary includes usage metadata.
- Backends without usage continue to omit usage fields.
- `cargo fmt --all --check`, `cargo test --workspace`, and `cargo run -p llmff -- inspect examples/json-repair.yaml` pass.
