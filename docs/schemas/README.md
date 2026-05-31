# Machine-Readable Contract Schemas

These JSON Schemas freeze the llmff core contract at version 1:

- `pipeline-manifest-v1.schema.json`: YAML or JSON pipeline manifests with `version: 1`.
- `event-v1.schema.json`: live lifecycle events emitted by `llmff run --events`.
- `trace-v1.schema.json`: JSONL trace records written by trace and event writers.
- `plugin-manifest-v1.schema.json`: `llmff-plugin.yaml` plugin manifests.
- `plugin-validation-report-v1.schema.json`: JSON output from `llmff plugins validate --format json`.
- `inspect-report-v1.schema.json`: JSON output from `llmff inspect --format json`.
- `run-result-v1.schema.json`: `result.json` written by `llmff run --run-dir`.
- `failure-kinds-v1.json`: stable `run_failed.failure_kind` values shared by
  lifecycle events, traces, and compatibility documentation.

Discovery JSON outputs that do not have dedicated schemas are fixture-backed
under `fixtures/golden/discovery/`: stages list, backends list, backends
report, models list, and plugins list. The fixture checker validates their
representative record shapes as part of `scripts/check-schema-contract.py`.

Schemas use JSON Schema draft 2020-12. YAML fixtures are loaded as normal JSON-compatible data before validation.

The multi-release compatibility matrix in
`docs/compatibility/core-contract-v1-matrix.json` records which release first
introduced or preserved each schema, event, trace, and CLI JSON surface. The
schema contract checker validates that matrix together with the schema files
and golden fixtures.

## Inline Graph Metadata

Pipeline manifests may include:

```yaml
metadata:
  inline_graph_syntax_version: 1
```

Inline graph syntax version `1` covers the existing CLI `--graph` form: stages separated by `|`, optional ids with `op#id`, positional values with `op(value)`, and key/value parameters with `op(key=value)`. This metadata documents the syntax version used to generate or mirror a manifest. It is optional and does not change existing inline graph behavior.
