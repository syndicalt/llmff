# OpenTelemetry Bridge Design

`llmff` does not emit OpenTelemetry data directly today. The future OpenTelemetry bridge
is a deployment-owned bridge that reads existing local artifacts and translates
them outside the `llmff run` process.

The stable source remains the file-based supervision contract:

- lifecycle events from `--events`
- trace records from `--trace`
- local summaries from `scripts/trace-to-summary.sh`
- Prometheus-style metrics from `scripts/trace-to-metrics.sh`

This keeps default execution boring: no collectors by default,
no network telemetry by default, no background agents, and no implicit
dependency on a vendor backend.

## Bridge Boundary

A bridge process may run after a trace is closed, or beside a supervisor that
already owns event consumption. The bridge should be launched by the deployment
or agent host, not by `llmff`.

Recommended flow:

```bash
llmff run pipeline.yaml --trace .llmff/runs/job-42/trace.jsonl
scripts/trace-to-summary.sh .llmff/runs/job-42/trace.jsonl
scripts/trace-to-metrics.sh .llmff/runs/job-42/trace.jsonl
```

The bridge can convert the summary and metrics into collector-specific spans,
metrics, or logs. That conversion is intentionally outside the current
contract.

## Attribute Mapping

This attribute mapping is the recommended starting point for bridge authors.

Use a conservative attribute mapping so future exporters remain additive:

| Source | Suggested attribute |
| --- | --- |
| `run_id` | `llmff.run.id` |
| `manifest_hash` | `llmff.manifest.hash` |
| `stage_id` | `llmff.stage.id` |
| `op` | `llmff.stage.op` |
| `model_alias` | `llmff.model.alias` |
| `backend_alias` | `llmff.backend.alias` |
| `failure_kind` | `llmff.failure.kind` |
| `duration_ms` | span duration or metric value |
| token usage fields | metric values |

Unknown trace and event fields must be ignored by bridge implementations until
they are explicitly mapped.

## Payload Exclusion

These payload exclusion rules keep telemetry metadata separate from user data.

The bridge must not read prompt bodies, model payloads, tool request bodies, or
declared output artifacts. Trace and event streams are metadata; output files
remain payload artifacts owned by the caller.

Allowed metadata includes stage names, operation names, timing, retry counts,
token usage, cache status, backend aliases, output artifact paths, and
sanitized failure fields.

## Support Commitment

Any built-in exporter is a support commitment, not just a generated example.

Shipping a built-in OpenTelemetry exporter, collector configuration, hosted
dashboard, or vendor-specific integration is a support commitment. Until that
commitment is accepted, the supported bridge point is the local exporter output
and the documented metadata contract above.
