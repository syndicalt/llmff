# Traces, Events, And Observability Without Prompt Logging

[IMAGE PLACEHOLDER: Header diagram showing a JSONL trace timeline with run events, stage duration bars, loop iteration context, token metadata, and a separate lane labeled "declared payload artifacts".]

LLM pipelines need observability. They do not need every prompt, tool body, provider payload, and model response dumped into the same log stream — and those two ideas get confused constantly.

The confusion has a familiar arc. A workflow is hard to inspect, so the default answer is more logging: print the prompt, print the tool request, print the provider response, print the repaired JSON, print whatever helps the next person debug the run. It works, and it keeps working right up until the log file is the most sensitive artifact in the entire system and nobody can remember deciding that it should be.

`llmff` starts from a narrower default: traces and lifecycle events are metadata streams, and payloads belong in declared outputs and caller-owned artifacts. A trace should tell you what happened without becoming a prompt dump.

## Two Streams, Different Jobs

`llmff` exposes two related observability surfaces:

```bash
llmff run examples/json-repair.yaml \
  --trace .llmff/runs/job-42/trace.jsonl \
  --events .llmff/runs/job-42/events.jsonl
```

`events.jsonl` is for lifecycle supervision — it tells a live caller that a run started, a stage started, a stage finished, or the run failed, which is what dashboards, subprocess supervisors, and shell pipelines need while the process is still running. `trace.jsonl` is for post-run inspection: the execution record a caller can summarize, turn into metrics, store next to a job record, or read while debugging a failure.

Both are JSONL, one JSON object per non-empty line, and consumers should ignore fields they don't recognize since new optional fields can appear over time. The schema-backed shape is intentionally simple:

```json
{
  "run_id": "fixture-run",
  "event": "stage_finished",
  "timestamp_ms": 1780000000034,
  "stage_id": "draft",
  "op": "infer",
  "status": "success",
  "duration_ms": 30,
  "model": "openai:gpt-test",
  "backend": "openai",
  "provider_model": "gpt-test",
  "attempts": 3,
  "prompt_tokens": 12,
  "completion_tokens": 8,
  "total_tokens": 20
}
```

That one record says plenty: which stage ran, which operation it used, how long it took, which model alias resolved to which backend and provider model, how many attempts happened, and what usage metadata the provider returned. What it doesn't include is the prompt — and that omission is a feature of the contract, not a missing debug knob.

## Metadata Is Enough For Many Questions

A supervisor rarely needs prompt bodies to answer its operational questions. Did the run start, which stage is active, which stage failed, and was the failure static validation, backend, timeout, local I/O, JSON, or stage execution? How long did each stage take, which backend was used, how many attempts happened, how many tokens were reported, which output artifact was written, and which loop iteration or map item produced this event? Every one of those is a metadata question.

For a successful local fixture, the whole trace can look like this:

```jsonl
{"run_id":"fixture-run","event":"run_started","timestamp_ms":1780000000000}
{"run_id":"fixture-run","event":"stage_started","timestamp_ms":1780000000001,"stage_id":"load_prompt","op":"load"}
{"run_id":"fixture-run","event":"stage_finished","timestamp_ms":1780000000003,"stage_id":"load_prompt","op":"load","status":"success","duration_ms":2}
{"run_id":"fixture-run","event":"stage_started","timestamp_ms":1780000000004,"stage_id":"draft","op":"infer"}
{"run_id":"fixture-run","event":"stage_finished","timestamp_ms":1780000000034,"stage_id":"draft","op":"infer","status":"success","duration_ms":30,"model":"openai:gpt-test","backend":"openai","provider_model":"gpt-test","attempts":3,"prompt_tokens":12,"completion_tokens":8,"total_tokens":20}
{"run_id":"fixture-run","event":"stage_started","timestamp_ms":1780000000050,"stage_id":"write_answer","op":"write"}
{"run_id":"fixture-run","event":"stage_finished","timestamp_ms":1780000000053,"stage_id":"write_answer","op":"write","status":"success","duration_ms":3,"output_path":"examples/out/answer.json"}
{"run_id":"fixture-run","event":"run_finished","timestamp_ms":1780000000054,"status":"success"}
```

A dashboard can draw a timeline from that. A queue worker can detect the final status, a cost monitor can aggregate token counts wherever providers supply usage, and a debugging script can sort slow stages by `duration_ms`. None of it requires a model input or output body anywhere in the trace.

## Payloads Belong In Declared Artifacts

Notice what the write stage's trace record actually contains:

```json
{
  "event": "stage_finished",
  "stage_id": "write_answer",
  "op": "write",
  "status": "success",
  "output_path": "examples/out/answer.json"
}
```

That's the clean split. The trace says a payload artifact was written; the payload itself lives where the manifest declared it should live. If the caller wants to store, encrypt, redact, index, or delete that payload, it does so under its own artifact policy. The trace never becomes a shadow payload store.

This matters more as pipelines get more useful, not less. A repair pipeline may handle private customer input. A tool loop may pass internal search results through a command. A summarization job may process meeting notes, and a batch classifier may touch hundreds of records. The observability layer should support operating those workflows without casually copying their contents into metadata.

To be clear, I'm not claiming metadata is non-sensitive in every environment — run IDs, paths, model names, token counts, and failure kinds can all matter somewhere. But metadata is a smaller, more manageable surface than raw prompts and responses, and smaller surfaces are easier to retain, export, and review.

## Failure Kinds Are For Supervisors

When a run fails, a supervisor needs a stable failure class more than it needs a wall of stderr:

```jsonl
{"run_id":"fixture-error","event":"run_started","timestamp_ms":1780000000100}
{"run_id":"fixture-error","event":"stage_started","timestamp_ms":1780000000101,"stage_id":"draft","op":"infer"}
{"run_id":"fixture-error","event":"run_failed","timestamp_ms":1780000000105,"status":"failed","failure_kind":"backend","failure_message":"backend request failed"}
```

The process exit code remains the final authority — if `llmff` exits non-zero, the host preserves that status. The failure kind is what helps decide the next move:

```text
graph_validation / manifest_parse / config -> fix manifest or invocation
io / json                              -> repair local files or malformed JSON
backend / http / timeout               -> retry later, switch backend, lower concurrency
stage_execution                        -> inspect the named stage boundary
interrupted                            -> preserve interruption status and resume only with a matching checkpoint
```

The `failure_message` is a deliberately safe operational summary — no prompts, no secrets, no tool bodies, no backend payloads, no provider response bodies. That's the right default for a runner meant to be embedded under other systems. The caller can keep richer payload evidence under its own policy; the runner's failure stream stays suitable for machine handling.

## Loop Context Without Log Scraping

Loops break naive observability. If a loop body contains a stage called `draft`, there may be five `draft` executions across iterations, and a log line that says "draft finished" tells you almost nothing. Loop body records carry their context instead:

```json
{
  "run_id": "cli-run",
  "event": "stage_finished",
  "stage_id": "refine_loop.draft",
  "op": "infer",
  "status": "success",
  "loop_id": "refine_loop",
  "loop_iteration": 1,
  "loop_stage_id": "draft",
  "duration_ms": 2409,
  "total_tokens": 128
}
```

That gives the supervisor two handles: `stage_id` as the fully qualified runtime identifier, and `loop_id` / `loop_iteration` / `loop_stage_id` mapping the event back to the manifest body. From there, ordinary operational views fall out directly:

```text
refine_loop iteration 1
  draft          success  2409ms  128 tokens
  critique       success   911ms   74 tokens
  quality_check  success     3ms
  quality_ready  success     1ms
  final_answer   success     1ms
```

The host never parses a prompt and never has to understand the loop body as code — the trace context is enough. Stage IDs are the bridge between manifests and traces.

## Map Context Uses The Same Discipline

Map stages need item context for the same reason loops need iteration context: a body stage named `extract_name` means little without knowing which item produced it.

```json
{
  "run_id": "cli-run",
  "event": "stage_finished",
  "stage_id": "map_records[2].extract_name",
  "op": "extract",
  "status": "success",
  "map_id": "map_records",
  "map_index": 2,
  "map_stage_id": "extract_name",
  "duration_ms": 1
}
```

With that, a caller can build a per-item dashboard, isolate a slow item, or correlate a failed body stage with an input index. It also means parallel execution doesn't force log archaeology: independent map items can finish in any order, and the supervisor still groups by `map_id` and `map_index`. The order of log arrival is not the data model.

## Local Exporters Keep The Boundary Plain

JSONL traces are useful raw, but most teams eventually want summaries and metrics. `llmff` keeps that local:

```bash
llmff run examples/json-repair.yaml --trace /tmp/llmff-trace.jsonl
scripts/trace-to-summary.sh /tmp/llmff-trace.jsonl
scripts/trace-to-metrics.sh /tmp/llmff-trace.jsonl
```

The summary exporter reports stage counts, per-stage timing, wall-clock duration, retry totals, token usage, cache hits, output paths, backend errors, timeout errors, and failure counts. The metrics exporter emits Prometheus-style text:

```text
llmff_stage_duration_ms_sum 45
llmff_stage_duration_ms{stage_id="draft",op="infer"} 30
llmff_prompt_tokens_total 12
llmff_completion_tokens_total 8
llmff_tokens_total 20
llmff_backend_errors_total 0
llmff_failures_total 0
```

These scripts use local files, Bash, and Python standard library modules. They don't start collectors, don't open network connections, and don't send telemetry to a vendor. That isn't a stance against OpenTelemetry — it's a clean boundary for it. A deployment can feed the metrics text into its own collector bridge, and the runner never has to become a telemetry agent.

## Events Are Live Evidence, Not Final Authority

Lifecycle events shine while a process is running:

```bash
llmff run pipeline.yaml \
  --events - \
  --trace .llmff/runs/job-42/trace.jsonl
```

A Node.js supervisor can consume each JSONL event from stdout, a Python worker can update a job row after `stage_finished`, a shell script can show progress in CI. But events are evidence, not authority — the process exit code is the authority. An event writer can fail. A host can kill the process before `run_failed` is emitted. A filesystem can fill, a process can be interrupted. So the supported pattern is: stream or store events, wait for process exit, preserve the exit code, and only then read the result and trace artifacts.

Observability should improve supervision. It should not replace the subprocess contract.

## What Not To Put In The Trace

The trace should not become a junk drawer. No full prompt bodies by default. No API keys, ever. No raw provider request or response payloads, no command stdin bodies, and no tool result bodies unless the field is explicitly a declared artifact under the caller's policy.

I understand the temptation — one file that answers every debugging question. But that file then becomes hard to retain, hard to share, hard to export, and hard to use in dashboards, which defeats the reasons you wanted observability in the first place. Keep the trace focused on who ran, what stage, which operation, which status, how long, which backend and model metadata, which attempt count, which token usage, which failure kind, which loop or map context, and which artifact path. That covers most operational work. For payload debugging, use payload artifacts with explicit retention and access policy.

## The Boring Shape Scales Better

The full observability contract fits in six lines:

```text
inspect.json  -> expected execution shape
events.jsonl  -> live lifecycle metadata
trace.jsonl   -> post-run execution metadata
result.json   -> final run summary
exit code     -> process outcome
outputs       -> declared payload artifacts
```

This is the shape I want because it composes. A shell can use it. A queue worker can use it. A CI job can use it. An agent host can use it without importing `llmff` as a framework, and a dashboard can use it without ever reading a prompt.

There's no need to hide the runner inside an application runtime just to observe it. The files are the interface, the process status is the authority, the trace is metadata, and the payloads stay where the manifest said they would be. Finite work, visible state — that's the operating model.
