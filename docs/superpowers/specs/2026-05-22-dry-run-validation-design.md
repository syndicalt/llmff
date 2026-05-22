# Dry-Run Validation Design

Add dry-run validation for pipeline manifests without invoking model backends, tools, file reads, file writes, or deterministic stage execution.

## User Shape

`llmff inspect <manifest>` should be the primary dry-run validation command. It should parse the manifest, validate graph references, validate built-in stage requirements, and verify that model-calling stages can resolve to a configured backend.

Backend configuration stays CLI-first:

```bash
llmff inspect pipeline.yaml --backend openai=https://api.openai.com/v1
llmff inspect pipeline.yaml --ollama ollama=http://localhost:11434
```

Environment variables remain optional secret indirection through `--api-key-env`; no backend should be registered implicitly from environment variables.

## Validation Scope

Dry-run validation must reject:

- Unknown stage operations.
- Missing required stage parameters.
- Missing configured backend for `infer` and `repair` model ids.

Required parameters by stage:

- `load`: `input`.
- `infer`: `from`, `model`.
- `validate_json`: `from`, and one of `schema` or `schema_path`.
- `system`: `from`.
- `template`: `from`, `path`.
- `repair`: `from`, `model`.
- `route`: `from`, and at least one target among `on_success`, `on_invalid`, `on_skipped`, `cases`, or `default`.
- `tool`: `from`, plus existing graph transport validation for `command` or `url`.
- `write`: `from`, plus existing graph path validation.

Dry-run validation must not:

- Call `Backend::infer`.
- Execute command tools.
- Make HTTP tool requests.
- Read input, template, schema, or system prompt files.
- Write outputs or trace files.

## Architecture

Keep `Graph::from_manifest` responsible for structural validation: stage ids, parent references, route target references, output references, and stage-specific graph rules already present for tool and write.

Add `Engine::validate_manifest(&self, manifest: Manifest) -> Result<Graph, LlmffError>`. The engine owns backend registry state, so it is the correct place to validate backend availability. `run_manifest_with_options` should use this same method before execution so run and inspect share the validation contract.

Update the CLI so `run` and `inspect` both build an `Engine` from the same explicit backend flags. This keeps inspect behavior aligned with run behavior while avoiding stage execution.

## Acceptance Criteria

- `llmff inspect examples/json-repair.yaml` still prints `ok`.
- `llmff inspect` rejects `model: openai:gpt-test` unless `--backend openai=<url>` is supplied.
- `llmff inspect --backend openai=http://127.0.0.1:1` accepts an OpenAI-compatible alias without making a network call.
- Core tests prove unknown operations and missing required stage parameters fail during engine validation.
- `cargo fmt --all --check`, `cargo test --workspace`, and `cargo run -p llmff -- inspect examples/json-repair.yaml` pass.
