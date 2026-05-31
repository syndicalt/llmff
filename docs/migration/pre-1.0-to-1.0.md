# Pre-1.0 To 1.0 Migration Checklist

This checklist is for users moving manifests, wrappers, or plugins from the
pre-1.0 release line toward the 1.0 contract. It is intentionally conservative:
pin what you run, inspect before executing, and depend only on documented
contracts.

## Pipeline Manifests

- Pin the `llmff` version used by CI, supervisors, and release jobs.
- Run `llmff inspect <manifest> --format json` for every production manifest
  and store the report with the caller's job record.
- Keep payload output paths explicit. Do not depend on trace, events, stderr,
  or `result.json` containing prompt bodies or final payloads.
- Replace ad hoc wrapper assumptions with documented exit-code and
  `failure_kind` handling.
- Prefer `--run-dir <dir>` for supervised runs so inspect, trace, events,
  checkpoint, and result metadata stay together.

## Agent And CI Wrappers

- Treat the `llmff` process exit code as the final success authority.
- Preserve non-zero exit codes when adapting results into an agent framework,
  queue worker, CI job, or scheduler.
- Read `run_failed.failure_kind`, `failure_message`, and `retry_recommendation`
  only as classification evidence after a non-zero exit.
- Handle unknown failure kinds additively by recording the value and falling
  back to the broad exit-code posture.
- Use `llmff doctor` for local preflight checks such as run-dir writability,
  plugin validation, and API-key environment wiring. Live provider calls remain
  explicit smoke tests.

## Plugins

- Validate plugin directories with `llmff plugins validate --plugin-dir <dir>`
  and, where useful, `llmff doctor --plugin-dir <dir>`.
- Keep plugin entrypoints executable and relative to the plugin manifest root.
- Depend on documented plugin capability kinds only: stages, backends,
  samplers, and tool transports.
- Treat plugin registry promotion and provider support labels as evidence
  based, not as implicit production certification.

## Provider Integrations

- Register provider aliases explicitly with `--backend alias=url` or
  `--ollama alias=url`.
- Use `--api-key-env alias=ENV_NAME` for secrets instead of embedding keys in
  manifests, traces, events, or wrapper logs.
- Run `llmff backends report` to review deterministic provider capabilities
  before live execution.
- Use opt-in smoke scripts for live endpoint certification.

## Compatibility Audit

Before declaring a wrapper 1.0-ready, run:

```bash
llmff inspect pipeline.yaml --format json
llmff doctor --run-dir .llmff/runs/preflight
llmff run pipeline.yaml --run-dir .llmff/runs/job-42
```

Then verify that the wrapper stores `inspect.json`, `trace.jsonl`,
`events.jsonl`, `checkpoint.json`, `result.json`, declared outputs, and the
original process exit code according to
[`docs/agent-harness-contract.md`](../agent-harness-contract.md).
