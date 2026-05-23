# Core Contract Compatibility v1

llmff core contract v1 covers pipeline manifests, inline graph syntax metadata, lifecycle events, traces, plugin manifests, plugin process protocol, and plugin validation reports.

## Pipeline Manifests

`version: 1` is the stable manifest version. Existing fields keep their meaning within this contract. Additive fields require a schema update and should not change the behavior of existing valid manifests.

Manifest `metadata.inline_graph_syntax_version: 1` records compatibility with the current inline graph CLI syntax. The field is documentation metadata for generated or equivalent manifests; runtime parsing of `llmff run --graph` remains unchanged.

## Inline Graph Syntax v1

Inline graph syntax v1 includes:

- Linear stages separated by `|`.
- Stage ids declared with `op#id`.
- Positional stage value syntax such as `template(prompt.tmpl)`.
- Key/value parameters such as `infer(model=mock:good,temperature=0)`.
- Existing parameters for sampling, retrieval, cache, write, and tool stages.

Future breaking syntax changes require a new inline graph syntax version.

## Events And Traces

Events and traces are newline-delimited JSON. Consumers should correlate records by `run_id` and `stage_id`, and should treat unknown future fields as additive.

Failure records use `event: "run_failed"` with `failure_kind` and `failure_message` when a writer is available.

## Plugin Protocol

Plugin protocol version `1` covers the manifest schema, capability kinds, entrypoint resolution, process stdin/stdout lifecycle, backend and sampler JSON payloads, and validation report shape.

Valid capability kinds are `backend`, `sampler`, `stage`, and `tool-transport`. Additive JSON fields may appear in future compatible releases. Breaking process or payload changes require a new plugin protocol version.
