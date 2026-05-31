# Core Contract Compatibility v1

llmff core contract v1 covers pipeline manifests, inline graph syntax
metadata, lifecycle events, traces, inspect reports, plugin manifests, plugin
process protocol, plugin validation reports, and run-result artifacts. The
broader v1.0 release surface is classified in `docs/v1-contract.md`; this
compatibility document is the machine-contract subset.

`docs/compatibility/core-contract-v1-matrix.json` is the machine-readable
multi-release proof for this contract. The schema contract gate checks that the
matrix covers `v0.1.3`, `v0.1.4`, and `v0.1.5`; roadmap functionality items 1
through 7; and the `schema`, `event`, `trace`, and `cli_json` compatibility
surfaces with additive-only policy and fixture evidence.

## Multi-Release Compatibility Matrix

Core contract v1 was introduced in `v0.1.3`, preserved through the roadmap
completion release in `v0.1.4`, and extended additively by the agent harness
`result.json` contract in `v0.1.5`.

| Release | Roadmap items | Schema | Events | Traces | CLI JSON |
| --- | --- | --- | --- | --- | --- |
| `v0.1.3` | 4, 5, 7 | Introduced v1 schemas and golden fixtures. | Introduced lifecycle JSONL schema and success/failure fixtures. | Introduced trace JSONL schema and stage fixtures. | Introduced inspect JSON and plugin validation JSON schemas and fixtures. |
| `v0.1.4` | 1, 2, 3, 4, 5, 6, 7 | Preserved v1 schemas under additive-only policy. | Added failure-classification expectations without changing existing event meanings. | Preserved trace JSONL for local observability and downstream dashboard fixtures. | Preserved inspect and plugin validation JSON for supervisors and ecosystem tooling. |
| `v0.1.5` | 1, 2, 7 | Added `run-result-v1.schema.json` while preserving existing schema surfaces. | Preserved lifecycle JSONL as the run-directory event artifact. | Preserved trace JSONL as the run-directory execution artifact. | Added schema-backed `result.json` fixtures for success, stage failure, and interruption. |

Compatibility evidence is intentionally checked through local files instead of
release downloads: release notes document the release-level promise, schemas
define the machine contract, and golden fixtures provide downstream consumers
with stable examples to validate against.

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

Events and traces are newline-delimited JSON. Consumers should correlate
records by `run_id` and `stage_id`. Within a specific v1 schema file, fields
are closed for fixture validation; future additive fields require schema and
fixture updates, and consumers should ignore fields they do not understand.

Trace `stage_finished` records may include `attempts` when a retryable stage
needed more than one attempt. Missing `attempts` means one attempt.

Failure records use `event: "run_failed"` with `failure_kind` and `failure_message` when a writer is available. Current `failure_kind` values are `manifest_parse`, `io`, `json`, `graph_validation`, `unknown_stage`, `timeout`, `http`, `stage_execution`, `backend`, `config`, `not_implemented`, and `interrupted`. New `failure_kind` values are additive compatibility changes and must be added to `docs/schemas/failure-kinds-v1.json`, trace/event schemas, fixtures, and docs together.

Process exit-code meanings are part of the CLI compatibility surface. New
non-zero codes may be added only when they describe a new broad failure class;
existing code meanings should not change within core contract v1.

Interrupted runs use exit code `130`. A signal can arrive before event writers
flush a final failure event, so supervisors should treat that exit code as the
authoritative interrupted-run outcome.

## Inspect Reports

`llmff inspect --format json` emits an inspect report with
`format_version: 1`. Existing fields keep their meaning within this contract.
Consumers should ignore unknown future fields and use the manifest hash,
schema compatibility versions, resolved inputs, resolved outputs, stage order,
stage capability constraints, backend registrations, plugin protocol metadata,
plugin manifests, requested execution controls, artifact paths, checkpoint
intent, and stdout ownership fields as preflight metadata rather than payload
logs.

## Plugin Protocol

Plugin protocol version `1` covers the manifest schema, capability kinds, entrypoint resolution, process stdin/stdout lifecycle, backend and sampler JSON payloads, and validation report shape.

Valid capability kinds are `backend`, `sampler`, `stage`, and `tool-transport`. Additive JSON fields may appear in future compatible releases. Breaking process or payload changes require a new plugin protocol version.
