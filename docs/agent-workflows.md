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

## Canonical Subprocess Patterns

Agents should prefer a caller-owned run directory for ordinary single-run
work. The run directory is the supported bundle for inspect, trace, events,
checkpoint, and result metadata:

```bash
llmff run --run-dir .llmff/runs/job-42 pipeline.yaml
```

Use explicit artifact flags when the agent needs a different stream owner or
when an older wrapper has not adopted `--run-dir`:

```bash
llmff run pipeline.yaml \
  --trace .llmff/runs/job-42/trace.jsonl \
  --events .llmff/runs/job-42/events.jsonl \
  --checkpoint .llmff/runs/job-42/checkpoint.json
```

Do not combine multiple stdout owners. If an agent streams lifecycle events
with `--events -`, manifest outputs and `--stream-stage` must write somewhere
else. If an agent streams a stage payload with `--stream-stage`, lifecycle
events must be file-backed.

### Short Jobs

For a bounded single request, allocate a unique run directory, run once, and
treat the process exit code as authoritative. `--run-dir` writes
`inspect.json`, `trace.jsonl`, `events.jsonl`, `checkpoint.json`, and
`result.json` under the directory:

```bash
llmff run --run-dir .llmff/runs/job-42 pipeline.yaml \
  --timeout-ms 30000 \
  --retry-attempts 2 \
  --retry-backoff-ms 250
```

The agent should read payload artifacts from declared output paths after exit
code `0`, not from trace, events, stderr, or result metadata.

### Long Jobs

For jobs that may be interrupted, use `--run-dir` or an explicit checkpoint
path from the first attempt. Resume only with a checkpoint produced by the same
manifest hash:

```bash
llmff run --run-dir .llmff/runs/job-42 pipeline.yaml \
  --timeout-ms 60000

llmff run --run-dir .llmff/runs/job-42 pipeline.yaml \
  --resume .llmff/runs/job-42/checkpoint.json \
  --timeout-ms 60000
```

The harness may also enforce its own host timeout around the subprocess. If the
host kills the process before `llmff` writes `run_failed`, preserve the host
status separately and use the absence of a final event as evidence only, not as
success.

### Batch Jobs

For batch work, keep item inputs and item outputs in explicit directories so
the agent can retry failed items without mixing payloads. Current `llmff`
behavior supports the run-directory metadata bundle while keeping batch input
and output paths explicit; batch mode can use `--run-dir` with explicit batch
paths:

```bash
llmff run --run-dir .llmff/runs/job-42 pipeline.yaml \
  --batch-input .llmff/runs/job-42/items.txt \
  --batch-output-dir .llmff/runs/job-42/batch-output \
  --timeout-ms 30000
```

Batch mode writes `batch-output/batch-report.jsonl` plus isolated item
artifacts under `batch-output/items/<index>/`. With `--run-dir`, `trace.jsonl`
records batch item lifecycle summaries and `checkpoint.json` records completed
item progress. If any item fails, `llmff` keeps the report, writes the
run-directory result summary, and exits non-zero after processing the batch.
The agent should preserve the original llmff exit code while using the report
to choose which items to repair or retry.

The runnable batch supervisor example performs an inspect preflight, writes a
line-based batch input, runs batch mode, summarizes the batch report, and
preserves the `llmff` process exit code:

```bash
python3 examples/agent-workflows/batch-supervisor.py
```

### Streaming Jobs

For live supervision, stream events only when manifest outputs write to files
and no stage payload is streamed to stdout:

```bash
llmff run pipeline.yaml \
  --events - \
  --trace .llmff/runs/job-42/trace.jsonl
```

For streamed model or stage payloads, keep lifecycle events file-backed:

```bash
llmff run pipeline.yaml \
  --stream-stage draft \
  --events .llmff/runs/job-42/events.jsonl \
  --trace .llmff/runs/job-42/trace.jsonl
```

Streaming does not change the success contract. The agent should wait for the
subprocess exit, preserve the original llmff exit code, and treat streamed
events or payload chunks as partial evidence until the process has exited.

## Artifact Ownership

The agent owns job identity, run-directory allocation, input materialization,
host timeout/cancellation, task-level retry policy, and retention policy.
`llmff` owns the contents of artifacts it writes:

| Artifact | Preferred writer | Agent use |
| --- | --- | --- |
| `inspect.json` | `llmff run --run-dir` or `llmff inspect --format json` | Preflight contract: manifest hash, stdout ownership, resolved inputs and outputs, execution controls. |
| `events.jsonl` | `llmff run --run-dir` or `--events` | Live lifecycle supervision. Safe metadata, not payload recovery. |
| `trace.jsonl` | `llmff run --run-dir` or `--trace` | Post-run debugging, summaries, and metrics. |
| `checkpoint.json` | `llmff run --run-dir` or `--checkpoint` | Resume state. Treat as sensitive because it can include stage values. |
| `result.json` | `llmff run --run-dir` | Final run summary with status, exit code, artifact names, failure kind, and retry recommendation. |
| `batch-report.jsonl` | `--batch-output-dir` batch mode | Per-item batch status and artifact paths. |
| declared outputs | manifest stages | Payload artifacts the agent may consume after successful completion. |

If `--run-dir` is used, do not also pass `--trace`, `--events`, or
`--checkpoint`; the CLI rejects that combination because the run directory owns
those paths. Batch mode may still pass `--batch-input` and `--batch-output-dir`
because those are payload item paths, not metadata paths.

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

- `graph_validation`, `manifest_parse`, `unknown_stage`, or `config`: fix the
  manifest or invocation before retrying
- `io` or `json`: repair local paths, permissions, files, or malformed JSON
  before retrying
- `backend`, `http`, or `timeout`: retry later, switch backend, or lower
  concurrency
- `stage_execution`: inspect the named stage and tool/backend configuration
- `interrupted`: preserve exit code `130` and resume only with a matching
  checkpoint and unchanged manifest hash

If `run_failed.failure_kind` is missing, unknown, or newer than the harness,
record the value when present, preserve the original llmff exit code, and fall
back to the exit-code posture above. Never translate a non-zero `llmff` status
into framework-level success merely because the harness understood the failure
kind.

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
