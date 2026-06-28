# Companion X Posts

1. A trace should tell you what happened without becoming a prompt dump.

For LLM pipelines, good observability is metadata first: stage ID, op, status, duration, backend, token usage, failure kind, loop/map context, and artifact paths.

2. Payloads belong in declared artifacts.

The trace can say `output_path: "answer.json"`.
The payload can live in `answer.json`.
That split keeps observability useful without turning every log into a sensitive data store.

3. Loop observability needs iteration context, not log scraping.

`loop_id`, `loop_iteration`, and `loop_stage_id` let a supervisor show progress through a bounded loop while keeping the body tied back to the manifest.

4. The process exit code is still the final authority.

Events are live evidence. Traces are post-run evidence. A supervisor should still wait for `llmff` to exit and preserve the original status.

5. Stage IDs are the bridge between manifests and traces.

The manifest declares `draft`.
The trace records `refine_loop.draft` with iteration context.
The supervisor can correlate behavior without parsing prompts.

6. Observability should not make the runner a telemetry agent.

`llmff` writes local JSONL traces and events. Exporters can turn those files into summaries or Prometheus-style metrics. The deployment owns where telemetry goes next.

