# Agent Workflows

`llmff` is useful in agent systems as a subprocess pipeline runner. The agent
keeps ownership of planning, memory, tool choice, and retry policy at the task
level; `llmff` owns one bounded inference pipeline with typed inputs, execution
controls, traces, and lifecycle events.

## Integration Contract

An agent supervisor should treat `llmff run` like any other external tool:

- pass the manifest path explicitly
- pass prompt input through `-i/--input`, manifest inputs, or batch input files
- keep payload output separate from lifecycle events
- write `--trace` for post-run inspection
- write or stream `--events` for live supervision
- use the process exit code as the final success or failure authority
- inspect `run_failed.failure_kind` and `run_failed.failure_message` only after
  a non-zero exit
- write `--checkpoint` for long jobs that should resume after interruption
- use `--resume` only with a checkpoint produced by the same manifest

Events and traces are metadata streams. They are safe for supervisors that must
avoid prompt bodies and model payloads, but they are not a payload log.

## Recommended Subprocess Shape

Use file-backed streams when the agent needs both payload output and event
monitoring:

```bash
llmff run pipeline.yaml \
  --trace .llmff/runs/job-42/trace.jsonl \
  --events .llmff/runs/job-42/events.jsonl \
  --checkpoint .llmff/runs/job-42/checkpoint.json \
  --timeout-ms 30000 \
  --retry-attempts 3 \
  --retry-backoff-ms 250
```

Use `--events -` when the supervisor wants lifecycle JSONL on stdout and the
manifest writes final payloads to files:

```bash
llmff run pipeline.yaml --events - --trace .llmff/runs/job-42/trace.jsonl
```

Do not combine multiple stdout owners. If an agent needs streamed stage payloads
with `--stream-stage`, write events to a file instead of `--events -`.

## Failure Handling

The exit code is the primary contract:

- `0`: the pipeline completed and declared outputs were written
- non-zero: the pipeline failed or a batch item failed

When events are available, use `failure_kind` to decide the agent response:

- `graph_validation`, `manifest_parse`, `schema`, or `config`: fix the
  manifest or invocation before retrying
- `backend`, `http`, or `timeout`: retry later, switch backend, or lower
  concurrency
- `stage_execution`: inspect the named stage and tool/backend configuration

`failure_message` is a sanitized operational summary. It intentionally omits
prompts, secrets, tool bodies, backend payloads, and provider response bodies.

## Checkpoints And Resume

Use checkpoints for long-running agent jobs:

```bash
llmff run pipeline.yaml --checkpoint .llmff/runs/job-42/checkpoint.json
```

If the process is interrupted, resume with the same manifest:

```bash
llmff run pipeline.yaml \
  --resume .llmff/runs/job-42/checkpoint.json \
  --checkpoint .llmff/runs/job-42/checkpoint.json
```

Checkpoints include stage values, so store them with the same care as other job
artifacts. A checkpoint is bound to the manifest hash and cannot be silently
reused after the graph changes.

## Runnable Supervisor Example

The local Python example runs the deterministic JSON repair pipeline, captures
events, writes a trace and checkpoint, exports a summary, and exits with the
same status as `llmff`:

```bash
python3 examples/agent-workflows/supervisor.py
```

To point the example at a development binary:

```bash
LLMFF_BIN=target/debug/llmff python3 examples/agent-workflows/supervisor.py
```

The example uses mock backend responses by default, so it does not need provider
credentials or network access.
