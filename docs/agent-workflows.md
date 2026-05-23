# Agent Workflows

`llmff` is useful in agent systems as a subprocess pipeline runner. The agent
keeps ownership of planning, memory, tool choice, and retry policy at the task
level; `llmff` owns one bounded inference pipeline with typed inputs, execution
controls, traces, and lifecycle events.

## Integration Contract

An agent supervisor should treat `llmff run` like any other external tool:

- call `llmff inspect --format json` before execution when the agent needs a
  reproducibility report or wants to validate stdout/artifact ownership
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

## Preflight Inspection

Use `inspect --format json` before dispatching a run when an agent needs a
machine-readable contract for the pipeline:

```bash
llmff inspect pipeline.yaml --format json
```

The report includes schema compatibility versions, the manifest hash, source
kind, resolved inputs and outputs, execution stage order, model aliases,
backend registrations, plugin protocol metadata, plugin manifests, stdout
ownership, and default execution controls. Agents can store this report next to
the trace and checkpoint to explain what was expected to run.

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

### Short Jobs

For a bounded single request, inspect first, run once, and treat the process
exit code as authoritative:

```bash
llmff inspect pipeline.yaml --format json > .llmff/runs/job-42/inspect.json
llmff run pipeline.yaml \
  --trace .llmff/runs/job-42/trace.jsonl \
  --events .llmff/runs/job-42/events.jsonl
```

The agent should read payload artifacts from declared output paths, not from
trace or event metadata.

### Long Jobs

For jobs that may be interrupted, add a checkpoint path on the first run and
reuse the same path with `--resume` only after verifying the manifest has not
changed:

```bash
llmff run pipeline.yaml \
  --checkpoint .llmff/runs/job-42/checkpoint.json \
  --trace .llmff/runs/job-42/trace.jsonl \
  --events .llmff/runs/job-42/events.jsonl
```

### Batch Jobs

For batch work, keep item inputs and item outputs in explicit directories so
the agent can retry failed items without mixing payloads:

```bash
llmff run pipeline.yaml \
  --batch-input .llmff/runs/job-42/items.jsonl \
  --batch-output-dir .llmff/runs/job-42/items \
  --trace .llmff/runs/job-42/trace.jsonl
```

The runnable batch supervisor example performs an inspect preflight, writes a
line-based batch input, runs batch mode, summarizes the batch report, and
preserves the `llmff` process exit code:

```bash
python3 examples/agent-workflows/batch-supervisor.py
```

### Streaming Jobs

For live supervision, stream events only when manifest outputs write to files:

```bash
llmff run pipeline.yaml \
  --events - \
  --trace .llmff/runs/job-42/trace.jsonl
```

For streamed model or stage payloads, keep lifecycle events file-backed:

```bash
llmff run pipeline.yaml \
  --stream-stage draft \
  --events .llmff/runs/job-42/events.jsonl
```

## Failure Handling

The exit code is the primary contract:

- `0`: the pipeline completed and declared outputs were written
- `2`: the CLI invocation is invalid, such as conflicting stdout owners,
  invalid flags, or unsupported batch options
- `10`: manifest, graph, configuration, or static validation failed before
  model/tool execution
- `20`: a stage or batch item failed during execution
- `21`: a backend, provider, HTTP tool, or timeout failure occurred
- `22`: local I/O or JSON processing failed
- `30`: the selected behavior is intentionally not implemented
- `130`: the process received an interrupt or termination signal before
  completion
- `1`: unclassified internal failure

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
reused after the graph changes. A manifest mismatch exits with code `10` and
reports the checkpoint path plus both manifest hashes, so an agent can stop
retrying and create a fresh checkpoint for the changed manifest.

## Runnable Supervisor Example

The local Python example runs the deterministic JSON repair pipeline, captures
an inspect JSON preflight report, captures events, writes a trace and
checkpoint, exports a summary, and exits with the same status as `llmff`:

```bash
python3 examples/agent-workflows/supervisor.py
```

To point the example at a development binary:

```bash
LLMFF_BIN=target/debug/llmff python3 examples/agent-workflows/supervisor.py
```

The example uses mock backend responses by default, so it does not need provider
credentials or network access.

## Node.js Streaming Supervisor

The Node.js example uses `child_process.spawn` so a JavaScript or TypeScript
agent host can consume lifecycle events while the process is still running. It
uses the same offline JSON repair fixture, inspect preflight, trace, checkpoint,
and exit-code preservation pattern as the Python supervisor:

```bash
node examples/agent-workflows/node-supervisor.mjs
```

To point the example at a development binary:

```bash
LLMFF_BIN=target/debug/llmff node examples/agent-workflows/node-supervisor.mjs
```

Use this shape when the agent host needs live JSONL event consumption from
stdout. Keep manifest payload outputs file-backed so stdout belongs to
`--events -`.
