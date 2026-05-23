# Agent Runner Adoption Guide

Use `llmff` as a bounded execution tool inside an agent system. The agent
decides the task, memory context, retry policy, and user-facing response.
`llmff` runs one explicit pipeline and returns process status, metadata, and
declared artifacts.

This guide is not a CLI reference. It describes the integration pattern an
agent host should implement.

## Control Loop

An agent runner should split ownership into five steps:

1. preflight: run `llmff inspect --format json` and store the report next to
   the job record.
2. dispatch: start `llmff run` with explicit input files, `--trace`,
   `--events`, and a timeout.
3. supervision: read lifecycle events separately from payload output and use
   the process exit code as the final authority.
4. artifact collection: collect declared output files, trace, events, summary,
   metrics, and optional checkpoint files.
5. retry decision: use `failure_kind`, exit code, timeout policy, and checkpoint
   status to decide whether the agent should retry, repair the manifest, switch
   providers, or stop.

## Reference Implementations

Start from the runnable examples:

```bash
python3 examples/agent-workflows/supervisor.py
python3 examples/agent-workflows/batch-supervisor.py
node examples/agent-workflows/node-supervisor.mjs
```

The Python supervisor shows the default subprocess pattern. It performs
preflight, captures events, writes a trace and checkpoint, preserves the
`llmff` exit code, and avoids reading prompt payloads from metadata.

The batch supervisor shows item isolation and report collection for queued
work. It writes explicit batch inputs, collects the batch report, checks item
artifact paths, and leaves retry decisions to the agent.

The Node supervisor shows live event supervision for JavaScript and TypeScript
hosts. Use it when the host needs streaming lifecycle events while the process
is still running.

## Job Record

Store these fields in the agent job record:

| Field | Source |
| --- | --- |
| manifest hash | `llmff inspect --format json` |
| stdout ownership | inspect report |
| input paths | agent dispatch plan |
| output paths | inspect report and manifest |
| trace path | `--trace` argument |
| events path or stream owner | `--events` argument |
| checkpoint path | `--checkpoint` argument |
| process exit code | subprocess status |
| failure kind | final `run_failed.failure_kind`, when emitted |
| artifact paths | declared outputs and batch item outputs |

Events, traces, summaries, and metrics are metadata. The runner must not read
prompt bodies, model payloads, tool request bodies, or final output artifacts
from metadata streams. The rule for agent hosts is simple:
do not read prompt payloads from metadata.

## Retry Decisions

Treat exit code as the first decision input:

- `0`: collect artifacts and mark the job complete.
- `2` or `10`: do not retry the same invocation; repair the CLI arguments,
  manifest, graph, checkpoint, or configuration.
- `20`: inspect `failure_kind` and stage context; retry only if the agent can
  change inputs, provider, timeout, or retry policy.
- `21`: retry later, switch provider, or reduce concurrency when the failure is
  provider or timeout related.
- `22`: treat as local file or JSON handling failure and repair the workspace.
- `130`: treat as interrupted; resume only with a matching checkpoint and the
  same manifest hash.

When `run_failed.failure_kind` is available:

- `graph_validation`, `manifest_parse`, `schema`, and `config` require manifest
  or invocation repair.
- `backend`, `http`, and `timeout` may be retried according to the agent's
  provider policy.
- `stage_execution` requires inspecting the stage contract and inputs before a
  retry.

## Long Jobs

For long jobs, always provide a checkpoint path:

```bash
llmff run pipeline.yaml \
  --checkpoint .llmff/runs/job-42/checkpoint.json \
  --trace .llmff/runs/job-42/trace.jsonl \
  --events .llmff/runs/job-42/events.jsonl
```

Resume only when the manifest hash still matches the checkpoint. A mismatch is
a new job, not a retry of the old one.

## Batch Jobs

For batch work, keep input lines and output directories explicit:

```bash
llmff run pipeline.yaml \
  --batch-input .llmff/runs/job-42/items.txt \
  --batch-output-dir .llmff/runs/job-42/batch-output
```

Use the batch report to identify failed items. Preserve item output directories
as artifacts so the agent can retry only the failed units.
