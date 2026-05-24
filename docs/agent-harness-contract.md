# Agent Harness Contract

`llmff` is a bounded FFmpeg-style subprocess runner for agents. An agent host
chooses the manifest, prepares explicit inputs, starts `llmff`, watches
metadata streams, and decides what to do next. `llmff` owns one pipeline run:
manifest validation, graph execution, declared artifacts, lifecycle events,
trace records, checkpoints, and process status.

`llmff` does not own task planning, memory, tool selection policy, human
approval, multi-agent coordination, or task-level retry strategy. Those remain
responsibilities of the agent harness.

## Process Contract

An agent harness should treat `llmff run` as a normal supervised subprocess:

1. Run `llmff inspect <manifest> --format json` before dispatch when the agent
   needs a machine-readable contract for the run.
2. Start `llmff run` with explicit inputs and file-backed metadata artifacts.
3. Keep stdout ownership unambiguous.
4. Wait for process exit and use the exit code as the final success authority.
5. Read `run_failed.failure_kind` only as additional failure classification
   when events or traces were written.
6. Collect declared output artifacts from manifest output paths, not from
   metadata streams.

The harness may enforce its own wall-clock timeout, cancellation policy,
provider budget, queue lease, and retry policy around the subprocess. `llmff`
should remain restartable and inspectable from files rather than requiring an
in-process embedding.

## Run Directory Layout

This branch adds the contract for:

```bash
llmff run <manifest> --run-dir <dir>
```

If the local binary does not yet expose `--run-dir`, use the equivalent
explicit flags:

```bash
llmff inspect <manifest> --format json > <dir>/inspect.json
llmff run <manifest> \
  --trace <dir>/trace.jsonl \
  --events <dir>/events.jsonl \
  --checkpoint <dir>/checkpoint.json
```

`--run-dir <dir>` is the canonical agent harness artifact directory. It should
create the directory when needed and write only run-scoped artifacts under it.
The intended layout is:

| Path | Owner | Purpose |
| --- | --- | --- |
| `inspect.json` | `llmff inspect` or harness preflight | Reproducibility report, manifest hash, stdout ownership, resolved inputs, resolved outputs, execution controls, and schema versions. |
| `events.jsonl` | `llmff run` | Live lifecycle metadata for supervision. Safe to tail while the process runs. |
| `trace.jsonl` | `llmff run` | Append-only post-run trace metadata for debugging, summaries, and metrics. |
| `checkpoint.json` | `llmff run` | Resume state for completed stages. Treat as sensitive job state because it can include stage values. |
| `result.json` | `llmff run` | Final run record containing exit code, status, selected failure kind, artifact paths, and retry recommendation. |
| `outputs/` | manifest stages or harness | Optional declared payload artifacts when the manifest writes under the run directory. |
| `batch/` | `llmff run` or harness | Optional batch item outputs and batch report when batch mode is used. |

`--run-dir` should not make metadata a payload log. Payload artifacts remain the
declared manifest outputs. If the manifest writes output paths outside the run
directory, `inspect.json` and `result.json` should record those paths rather
than copying payloads implicitly.

## Stdout Ownership

Only one stream may own stdout for a run:

- lifecycle events, with `--events -`;
- one streamed stage payload, with `--stream-stage <stage_id>`;
- a manifest output that writes to `"-"`;
- normal human-readable CLI output.

Agent harnesses should prefer file-backed metadata and payload artifacts:

```bash
llmff run <manifest> --run-dir <dir>
```

or the explicit equivalent:

```bash
llmff run <manifest> \
  --events <dir>/events.jsonl \
  --trace <dir>/trace.jsonl
```

Use `--events -` only when the agent host needs live lifecycle JSONL on stdout
and all payloads write to files. Do not combine `--events -` with streamed
stage output or manifest output to stdout. The inspect report's stdout
ownership fields are the preflight source of truth for deciding whether a
planned invocation is valid.

## Exit-Code Semantics

The process exit code is the authoritative outcome:

| Code | Meaning | Harness posture |
| --- | --- | --- |
| `0` | Run or inspection completed successfully. | Collect declared artifacts and mark the job complete. |
| `1` | Unclassified internal failure. | Treat as terminal unless a higher-level operator policy allows a bounded retry. |
| `2` | Invalid CLI invocation or unsupported option combination. | Do not retry unchanged; repair harness arguments. |
| `10` | Manifest, graph, configuration, checkpoint, or static validation failure before execution. | Do not retry unchanged; repair manifest, inputs, config, or checkpoint. |
| `20` | Stage execution failure or batch item failure. | Retry only when the agent can change inputs, timeout, provider, or retry policy. |
| `21` | Backend, provider, HTTP tool, or timeout failure. | Retry according to provider policy, switch backend, or reduce concurrency. |
| `22` | Local I/O or JSON processing failure. | Repair filesystem, permissions, paths, workspace state, or malformed JSON. |
| `30` | Selected behavior is intentionally not implemented. | Treat as terminal for this invocation and choose a different capability. |
| `130` | Interrupted or terminated before completion. | Resume only with a matching checkpoint and unchanged manifest hash. |

Events and traces can be missing when the process fails before writers open or
when the host kills the process. A `run_failed` event is useful evidence, but
it does not replace the process status.

## Artifact Separation

`inspect`, `trace`, `events`, `checkpoint`, and `result` have different jobs:

- `inspect.json` is a preflight contract. It describes what should run and what
  artifacts should be produced. It is not proof that execution happened.
- `events.jsonl` is a live supervision stream. It is optimized for tailing,
  dashboards, and state transitions while the process is running.
- `trace.jsonl` is post-run execution evidence. It is used for summaries,
  metrics, debugging, and compatibility fixtures.
- `checkpoint.json` is resume state. It is bound to the manifest hash and must
  not be reused after the graph changes.
- `result.json` is the final run status record. It summarizes the subprocess
  status and points to artifacts without embedding prompt bodies,
  model payloads, tool bodies, or secrets.

The harness should store all five beside the agent job record when available.
Consumers should ignore unknown JSON fields in `inspect`, `events`, and
`trace` artifacts because those schemas evolve additively within the current
contract.

## Failure Kinds And Retry Posture

When a `run_failed` event or trace record is available, `failure_kind` gives a
stable machine-readable class. Current values are `manifest_parse`, `io`,
`json`, `graph_validation`, `unknown_stage`, `timeout`, `http`,
`stage_execution`, `backend`, `config`, and `not_implemented`.

Recommended harness behavior:

- `manifest_parse`, `graph_validation`, `unknown_stage`, and `config`: repair
  the manifest, graph, static inputs, or configuration before retrying.
- `io` and `json`: repair local files, paths, permissions, or malformed JSON
  before retrying.
- `backend`, `http`, and `timeout`: apply the harness provider policy; bounded
  retry, provider switch, lower concurrency, or longer timeout can be valid.
- `stage_execution`: inspect the named stage, declared inputs, and tool/backend
  configuration; retry only after changing a relevant condition.
- `not_implemented`: choose a different pipeline or capability; retrying the
  same invocation is not useful.

New failure kinds are additive. Harnesses should handle unknown values by
recording them, preserving the original exit code, and falling back to the
broad exit-code posture.

## Safe Metadata Handling

Metadata artifacts are designed for supervision, not payload recovery. A
harness must not depend on events, traces, summaries, metrics, stderr, or
failure messages containing raw prompts, secrets, tool request bodies, backend
payloads, provider response bodies, or final model outputs.

Safe metadata may include stage ids, operation names, duration, provider model
aliases, token counts, cache paths, output paths, retry attempts, failure kind,
and sanitized failure messages. Checkpoints and declared outputs can contain
payload values, so store them under the agent system's normal job-artifact
retention and access controls.

Credentials should come from the environment, local provider configuration, or
the agent host's secret manager. They should not be embedded in manifests,
result records, trace files, event streams, or inspect reports.

## Adapter Pattern

OpenAI Agents SDK, LangGraph, and similar systems should wrap `llmff` as a
subprocess tool rather than importing it as the agent runtime.

The adapter should:

1. Allocate a unique run directory for the agent step.
2. Materialize explicit input files and choose a pinned manifest.
3. Run `llmff inspect --format json` and store `inspect.json`.
4. Spawn `llmff run <manifest> --run-dir <dir>` or the explicit trace, events,
   and checkpoint flags.
5. Stream `events.jsonl` into the framework's step state when live progress is
   needed.
6. Wait for process exit and read `result.json`.
7. Return a compact tool result to the agent containing status, artifact paths,
   failure kind, and safe summaries only.

The adapter should not feed trace or event payloads back into the model as if
they were user-visible results. If the agent needs the model output, it should
read the declared output artifact after exit code `0` or after a deliberate
operator-approved partial-failure policy.

For LangGraph, model this as a node that writes run artifacts and returns a
small state update. For the OpenAI Agents SDK, model it as a tool function that
spawns the process, supervises files, and returns a structured tool result.
Both patterns keep `llmff` as the bounded execution substrate and leave graph
control, memory, and user-facing reasoning in the host framework.
