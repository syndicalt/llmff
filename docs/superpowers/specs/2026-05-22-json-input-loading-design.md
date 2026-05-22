# JSON Input Loading Design

## Goal

Make `load` support explicit JSON inputs, matching the original stage contract that `load` can read text or JSON from a path or stdin.

## User Model

Manifest inputs may declare an optional `format`:

```yaml
inputs:
  payload:
    path: ./payload.json
    format: json
```

Supported formats are:

- `text`: default when `format` is omitted.
- `json`: parse the input source into `Value::Json`.

The format is explicit. `llmff` does not infer JSON from file extensions or content because prompts can legitimately be JSON-looking text, and hidden sniffing would make pipeline behavior harder to reproduce.

## Runtime Semantics

- `load` reads the configured input path exactly as it does today, including `-` for stdin.
- `format: text` returns `StageStatus::Success(Value::Text(source))`.
- `format: json` parses the source with `serde_json::from_str` and returns `StageStatus::Success(Value::Json(value))`.
- Invalid JSON fails the run with a `StageExecution` error naming the load stage and the input id.
- Unknown formats are validation errors during `inspect` and before runtime execution.

## Static Typing

Dry-run type validation should use declared input formats:

- A `load` stage whose input is `format: json` has success kind `Json`.
- A `load` stage whose input is omitted or `format: text` has success kind `Text`.
- Field routes from JSON loads are accepted.
- Field routes from text loads continue to be rejected.

This preserves the conservative validation model while making typed input data useful without an extra `validate_json` stage.

## Scope

Included:

- Manifest parsing for input `format`.
- Validation for supported input formats.
- Runtime JSON parsing in `load`.
- Type inference for `load` stages based on their referenced input format.
- README documentation and focused tests.

Excluded:

- Format inference from file extensions or content.
- YAML, TOML, CSV, binary, multimodal, or message input formats.
- Inline graph syntax for JSON load format. Inline graphs keep their current text default.

## Acceptance Criteria

- `Manifest::from_yaml_str` parses `inputs.*.format`.
- `Engine::validate_manifest` rejects unsupported input formats.
- `llmff inspect` accepts a field route whose source is a JSON load.
- `llmff run` can load JSON input, route by a JSON scalar field, and write the selected output.
- `llmff run` fails with a clear error when `format: json` input is invalid JSON.
- `cargo fmt --all --check`, `cargo test --workspace`, and `cargo run -p llmff -- inspect examples/json-repair.yaml` pass.
