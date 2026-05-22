# Chat Messages Design

## Goal

Preserve chat role structure through `system` and model inference stages instead of flattening system policy text into the user prompt for chat-capable backends.

## Rationale

The original pipeline-runner design calls out `Messages` as a core value type and asks backend adapters to support chat-style requests. `llmff` currently sends every model call as one user message, even after a `system` stage. That loses role metadata and makes the graph less faithful to modern inference APIs.

## Behavior

- Add a `Message` value type with `role` and `content`.
- Add `Value::Messages(Vec<Message>)`.
- A `system` stage with a `path` converts text or JSON input into two messages:
  - `system`: file contents.
  - `user`: the input rendered as text.
- A `system` stage without `path` preserves the input as it does today.
- `infer` sends messages to the backend. Text and JSON parents are wrapped as one user message.
- `repair` sends its generated repair instruction as one user message.
- OpenAI-compatible and Ollama backends serialize `InferRequest.messages` directly.
- Existing text pipelines keep working. `write`, `template`, `tool`, and JSON validation can still consume messages by rendering them to a conservative text representation where needed.

## Non-Goals

- No multimodal message parts yet.
- No provider-specific system prompt fallback for completion-only APIs.
- No user-authored YAML message arrays yet.
- No streaming changes.

## Verification

- Unit test proves `system` creates `Messages` when a policy file is present.
- Engine test proves an inferred backend receives system and user messages separately.
- Backend HTTP tests prove OpenAI-compatible and Ollama requests preserve message roles.
- CLI smoke tests and example inspection remain green.
