# Sampling Parameters Design

## Goal

Expose common model sampling controls in manifests and inline graphs so `llmff` recipes can describe generation behavior explicitly.

## User Model

Model-calling stages support:

```yaml
graph:
  - id: draft
    op: infer
    from: prompt
    model: openai:gpt-test
    temperature: 0.2
    top_p: 0.9
    max_tokens: 256
```

The same keys are supported in inline graphs:

```bash
llmff run -g 'load | infer(model=mock:good,temperature=0.2,top_p=0.9,max_tokens=256) | write(-)'
```

`repair` uses the same sampling fields because it is also a model-calling stage.

## Runtime Semantics

- `temperature`, `top_p`, and `max_tokens` are optional.
- Omitted parameters are not sent to providers except for existing provider-required defaults.
- `InferRequest` carries the optional values from the stage to the backend.
- OpenAI-compatible chat completions receive `temperature`, `top_p`, and `max_tokens` fields only when present.
- Ollama receives present sampling controls under `options`:
  - `temperature`
  - `top_p`
  - `num_predict` for `max_tokens`
- Mock backends accept the values without changing deterministic responses.

## Validation

Manifest deserialization owns numeric type checks. Additional semantic validation rejects:

- `temperature < 0`
- `top_p < 0` or `top_p > 1`
- `max_tokens == 0`

These checks run during `inspect` and before execution.

## Scope

Included:

- Manifest fields for `top_p` and `max_tokens`.
- Inline graph parsing for `top_p` and `max_tokens`.
- Core request propagation for `infer` and `repair`.
- OpenAI-compatible and Ollama request mapping.
- README documentation and focused tests.

Excluded:

- Provider-specific penalties, seeds, stop sequences, JSON response hints, streaming controls, and tool-choice controls.
- Validation of provider-specific upper bounds.

## Acceptance Criteria

- Manifests parse `top_p` and `max_tokens`.
- `inspect` rejects invalid sampling parameters.
- `infer` and `repair` pass sampling values to the backend request.
- OpenAI-compatible request JSON includes present sampling fields.
- Ollama request JSON maps `max_tokens` to `options.num_predict`.
- Inline graph syntax parses `top_p` and `max_tokens`.
- `cargo fmt --all --check`, `cargo test --workspace`, and `cargo run -p llmff -- inspect examples/json-repair.yaml` pass.
