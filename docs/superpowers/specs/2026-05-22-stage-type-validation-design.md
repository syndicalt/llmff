# Stage Type Validation Design

## Purpose

Add the first production slice of static stage type compatibility validation. The roadmap calls for graph validation to catch type mismatches before execution. Today `llmff inspect` validates references, required parameters, and backend availability, but some impossible graphs still pass dry-run validation and fail only at runtime.

This slice keeps the type system intentionally small and conservative. It rejects only mismatches that can be proven from current built-in stage contracts.

## Initial Type Model

Add a dry-run-only value kind model:

- `Text`: a successful stage output is text.
- `Json`: a successful stage output is structured JSON.
- `Any`: the successful output cannot be known statically.

This is not a replacement for runtime `Value`. It is a validation model used by `Engine::validate_manifest`.

## Stage Contracts

Initial success output kinds:

- `load`: `Text`.
- `system`: `Text`.
- `template`: `Text`.
- `infer`: `Text`.
- `validate_json`: `Json` on success.
- `repair`: `Text` because it calls a model only for invalid input and otherwise forwards a status.
- `route`: `Any` because selected target status can vary.
- `tool`: `Text`.
- `write`: forwards its parent kind.

Initial input compatibility rules:

- Text-compatible stages (`system`, `template`, `infer`, `tool`, `write`) remain permissive because current runtime behavior can stringify JSON or serialize values.
- `validate_json` accepts `Text` and `Json`.
- Status-based `route` accepts any parent status.
- Field-based `route` requires its `from` source to have success output kind `Json`.

## Validation Scope

`Engine::validate_manifest` should:

- Build the graph in dependency order.
- Apply existing operation, parameter, and backend checks.
- Track the successful output kind of each stage.
- Reject field-based route stages when `from` is known to be `Text`.
- Include the source stage id, route stage id, expected kind, and actual kind in the error.

`Graph::from_manifest` remains structural. Type compatibility belongs in the engine validator because it is part of stage semantics.

## Non-Goals

- Do not add `Messages`, `BinaryRef`, or new runtime `Value` variants in this slice.
- Do not reject all text-to-JSON validation paths; model output is text today and `validate_json` intentionally parses text.
- Do not infer JSON Schema shapes.
- Do not add plugin-provided type contracts yet.

## Acceptance Criteria

- `Engine::validate_manifest` rejects a field route whose `from` source is a `load` stage.
- `Engine::validate_manifest` accepts a field route whose `from` source is a `validate_json` stage.
- `llmff inspect` rejects the same invalid field-route graph.
- Existing example manifests still inspect and run.
- `cargo fmt --all --check`, `cargo test --workspace`, and `cargo run -p llmff -- inspect examples/json-repair.yaml` pass.
