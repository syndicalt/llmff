# Observability

`llmff` exposes observability through local JSONL traces and lifecycle event
streams. These streams are append-only metadata records and are safe for
supervisors, dashboards, and shell pipelines that must not read prompt bodies or
model payloads.

## Local Exporters

Use the local exporters when you need summaries without external services:

```bash
llmff run examples/json-repair.yaml --trace /tmp/llmff-trace.jsonl
scripts/trace-to-summary.sh /tmp/llmff-trace.jsonl
scripts/trace-to-metrics.sh /tmp/llmff-trace.jsonl
```

`scripts/trace-to-summary.sh` prints:

- run status
- stage counts and per-stage timing
- run wall-clock duration and total stage duration
- prompt, completion, and total token usage
- cache hits, misses, and hit rate
- backend and timeout error counts and rates
- failure counts and stable failure classes

`scripts/trace-to-metrics.sh` prints Prometheus-style text metrics:

- `llmff_stage_duration_ms_sum`
- `llmff_run_duration_ms`
- `llmff_stage_duration_ms{stage_id,op}`
- `llmff_prompt_tokens_total`
- `llmff_completion_tokens_total`
- `llmff_tokens_total`
- `llmff_cache_hits_total`
- `llmff_cache_misses_total`
- `llmff_cache_hit_rate`
- `llmff_backend_errors_total`
- `llmff_backend_error_rate`
- `llmff_failures_total`
- `llmff_failure_rate`
- `llmff_timeout_errors_total`
- `llmff_timeout_error_rate`

The scripts use only Bash and Python standard library modules. They do not open
network connections and do not require collectors, agents, or cloud services.

## Fixtures

Published fixtures live under `examples/supervision/fixtures/`:

- `event.schema.json`: JSON Schema for one lifecycle event.
- `success-trace.jsonl`: successful run with timing, usage, and cache metadata.
- `backend-error-trace.jsonl`: failed run with `failure_kind=backend`.

Consumers should ignore unknown fields. New optional fields can appear in minor
versions.

## OpenTelemetry Hook

The metrics exporter is the stable local hook for OpenTelemetry integration.
Deployments can run it after a trace is written and feed the resulting text into
their collector-specific bridge. `llmff` intentionally does not start a
collector or send telemetry over the network by default.

## Supervisor Contract

Supervisors should:

- correlate stages by `run_id` and `stage_id`
- keep stdout ownership clear by separating `--events`, `--trace`, and stage
  payload output
- treat `run_failed.failure_kind` as a stable failure class
- use the process exit code as the final authority for run failure
- store traces and events as metadata, not as a substitute for payload logs

For a complete agent-oriented subprocess pattern, see
[`docs/agent-workflows.md`](agent-workflows.md) and
[`examples/agent-workflows/supervisor.py`](../examples/agent-workflows/supervisor.py).
