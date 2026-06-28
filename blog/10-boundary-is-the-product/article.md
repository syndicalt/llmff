# The Boundary Is The Product

[IMAGE PLACEHOLDER: Clean architecture stack. Top layer: supervisor or agent host owns planning, memory, policy, approval, scheduling. Middle layer: llmff owns manifest execution, typed artifacts, trace/events, result, exit code. Bottom layer: models, tools, files, plugins. Highlight the subprocess boundary around the middle layer.]

The easiest way to ruin a small execution runner is to keep saying yes. Yes to planning, yes to memory, yes to scheduling, yes to human approval, yes to agent hosting, yes to tool policy — yes to every feature that seems adjacent because some LLM workflow somewhere needs it. None of those yeses feels like a mistake at the time. But enough of them and the runner stops being a runner; it becomes a half-framework with unclear ownership, and unclear ownership is the thing operators pay for later.

`llmff` takes the other side of that trade. It executes a declared bounded pipeline and leaves evidence. That's the job. The boundary is not a lack of ambition — it's the product shape, and this last article in the series is about why I believe that.

## The Middle Layer

`llmff` sits between the systems that decide why work should happen and the systems that perform individual calls. Above it live application code, agent hosts, queue workers, PM and task systems, memory systems, human approval flows, provider budget policy, and cross-job retry and scheduling policy. Below it live model providers, command tools, HTTP tools, plugin transports, files, local caches, and schemas.

In the middle: a manifest, a graph of typed stages, an inspect report, a supervised subprocess, events, traces, checkpoints, declared outputs, a result record, and an exit code.

That's a narrow surface, but don't mistake narrow for small. It's precisely the part that most LLM systems currently bury inside application code, where nobody can inspect it, version it, or supervise it from outside.

## The Supervisor Sequence

A good caller treats `llmff` like a normal executable:

```text
inspect -> run -> preserve exit code -> store trace/events/output -> decide next step
```

In shell form:

```bash
llmff inspect pipeline.yaml --format json > .llmff/runs/job-42/inspect.json

llmff run pipeline.yaml \
  --run-dir .llmff/runs/job-42

status=$?
printf '%s\n' "$status" > .llmff/runs/job-42/exit-code.txt
```

That small sequence carries the entire product philosophy. `inspect` tells the caller what is about to run — manifest hash, graph order, stdout ownership, input and output paths, backend aliases, schema versions, execution controls, and bounds for loop or map stages. `run` executes the declared graph. The exit code is the final process outcome, and the artifacts explain the run without requiring the supervisor to import an in-process callback system.

And then the caller decides the next step. That last sentence is load-bearing — everything else in the design exists so it can stay true.

## The Caller Owns Why

The caller knows the job. It knows whether this is a customer ticket, a CI check, an eval run, a support triage workflow, a queued background job, or one step in an agent plan. It knows the user, the tenant, the deadline, the budget, and the approval policy. `llmff` shouldn't pretend to know any of that, so the division of labor follows directly.

The caller owns task selection, planning, memory retrieval, user permissions, human approval, queue leasing, retry posture across jobs, provider budget policy, and artifact retention. `llmff` owns manifest parsing, static graph validation, dependency-ordered execution, typed stage values, declared failure paths, trace and event metadata, checkpoint compatibility, output writing, and process status.

The caller owns why. `llmff` owns what ran. The split sounds plain because it is plain — and plain boundaries survive contact with production systems far better than clever ones.

## Concrete Interface: Inspect

The preflight report is the first piece of the boundary:

```bash
llmff inspect examples/loops/map-batch-items.yaml --format json
```

```json
{
  "format_version": 1,
  "manifest": {
    "hash": "sha256:063a82ec87066c875315dde4bd6419b174fc96b8ee33809d389fc789cb3dbb2d",
    "version": 1
  },
  "execution": {
    "scheduler": "sequential",
    "max_concurrency": null,
    "stdout": {
      "events": false,
      "manifest_outputs": false,
      "stream_stage": false
    }
  },
  "stage_order": ["load_payload", "names"]
}
```

For bounded collection stages, the same report exposes the math:

```json
{
  "id": "names",
  "op": "map",
  "map": {
    "items_from": "items",
    "max_items": 3,
    "body_stage_count": 1,
    "max_expanded_stage_count": 3,
    "parallel": false
  }
}
```

It's worth being precise about what this is: a contract preview, not execution evidence — nothing has run yet. The caller can reject the job before dispatch if stdout ownership is wrong, the bounds are too high, a backend alias is missing, or the graph shape violates policy. And that's exactly where the runner stops. It reports what the manifest declares; it has no opinion about whether the business wants this job.

## Concrete Interface: Run Directory

The canonical supervised run is:

```bash
llmff run pipeline.yaml --run-dir .llmff/runs/job-42
```

```text
.llmff/runs/job-42/
  inspect.json
  events.jsonl
  trace.jsonl
  checkpoint.json
  result.json
  outputs/
```

Each file does one job. `inspect.json` is preflight — what should run. `events.jsonl` is live lifecycle metadata, safe to tail while the process is running. `trace.jsonl` is post-run execution evidence: stage IDs, operation names, status, duration, provider metadata, failure kind, loop and map context. `checkpoint.json` is resume state, and it deserves the same handling as sensitive job state because it can include stage values. `result.json` is the final run record, pointing at artifacts and summarizing the subprocess outcome. Declared outputs are payload artifacts, kept apart from the metadata streams.

That separation is itself part of the product. A trace should explain a run without becoming a prompt dump. A result record should summarize status without embedding secrets. A checkpoint should be handled like job state, not like a log line.

## Concrete Interface: Exit Codes

Exit codes give supervisors the most familiar control surface a process can offer:

```text
0   success
2   invalid CLI invocation
10  manifest, graph, config, checkpoint, or static validation failure
20  stage execution failure or batch item failure
21  backend, provider, HTTP tool, or timeout failure
22  local I/O or JSON processing failure
30  intentionally not implemented
130 interrupted
```

What to do with each code belongs above the runner, and different hosts will rightly disagree. A queue worker may retry `21` against a different provider. A CI job may treat `10` as a hard failure and send the manifest back to its author. An agent host may surface `20` to a planner along with the failed stage ID and a safe summary, and a human approval system may pause before any retry at all. `llmff` classifies the subprocess outcome. It does not become the organization's incident policy.

## Concrete Interface: Manifests And Schemas

The manifest is where intent becomes an execution contract:

```yaml
version: 1
inputs:
  question:
    path: question.txt
graph:
  - id: load_question
    op: load
    input: question
  - id: draft
    op: infer
    from: load_question
    model: mock:good
    response_format: json
  - id: validate
    op: validate_json
    from: draft
    schema_path: answer.schema.json
outputs:
  final:
    from: validate
    path: answer.json
```

A caller can check that file into source control, review changes to it, inspect the graph, pin the schemas, and compare manifest hashes across runs. The model call is not the unit of reproducibility — the declared run is.

This is why the runner has to resist features that erode the manifest boundary, even appealing ones. If a planner silently rewrites the graph at runtime, the caller no longer holds the contract it reviewed. If memory hides inside the runner, inputs are no longer explicit. If tool selection policy lives in the runner, the manifest stops being an honest description of what can happen. Some systems genuinely need those capabilities — and they should own them above the boundary, where they can be governed.

## What Refusal Buys

Non-goals aren't branding; they're engineering constraints, and each one purchases something specific. Because `llmff` doesn't own planning, the run stays finite. Because it doesn't own memory, inputs stay explicit. Because it doesn't own scheduling, queue workers keep their own leases, priorities, and retry windows. Because it doesn't own human approval, products enforce approval in the layer that actually knows users and permissions. And because it doesn't host agents, agent frameworks can call it without being replaced by it.

The FFmpeg analogy earns its place here one more time, with its limits acknowledged. FFmpeg doesn't decide why a video exists; it executes a declared media operation and emits files, logs, and a process status, and that shape composes with everything around it. `llmff` aims for that kind of boring — not boring as in weak, but boring as in inspectable, scriptable, restartable, and supervisable.

## Where The Boundary Can Feel Annoying

I won't pretend the boundary is free. You have to materialize inputs. You have to decide where artifacts live. You have to write a manifest instead of letting a framework hide the graph in code, think about stdout ownership, and preserve exit codes. If you want memory, you retrieve it before the run and pass it in as an explicit input. For a tiny experiment, that's real ceremony, and a notebook will beat it every time.

For supervised systems, the ceremony is the point. The files are the audit surface, the manifest is the review surface, the exit code is the control surface, and the trace is the debugging surface. When something fails at 2:00 a.m., hidden elegance is not useful. A run directory is.

## How Agents Should Use It

An agent host should wrap `llmff` as a subprocess tool, and the adapter can be unapologetically boring:

```python
import json
import subprocess
from pathlib import Path

run_dir = Path(".llmff/runs/job-42")
run_dir.mkdir(parents=True, exist_ok=True)

inspect = subprocess.run(
    ["llmff", "inspect", "pipeline.yaml", "--format", "json"],
    text=True,
    capture_output=True,
    check=False,
)
if inspect.returncode != 0:
    raise RuntimeError(inspect.stderr)

(run_dir / "inspect.json").write_text(inspect.stdout, encoding="utf-8")
contract = json.loads(inspect.stdout)

completed = subprocess.run(
    ["llmff", "run", "pipeline.yaml", "--run-dir", str(run_dir)],
    text=True,
    capture_output=True,
    check=False,
)

result = {
    "status": "ok" if completed.returncode == 0 else "failed",
    "exit_code": completed.returncode,
    "inspect": str(run_dir / "inspect.json"),
    "trace": str(run_dir / "trace.jsonl"),
    "events": str(run_dir / "events.jsonl"),
    "result": str(run_dir / "result.json"),
}
```

The agent receives a compact result — status, paths, failure kind when available, safe summaries — and needs nothing more. It doesn't need raw prompts in the event stream, doesn't need the runner to remember the conversation, and doesn't need `llmff` to become its planner. The agent plans; `llmff` executes the bounded step. That's composition, not competition.

## What This System Is Not

`llmff` is not an agent framework, a model server, a scheduler, a memory system, a vector database, a human approval product, a provider account manager, or a replacement for application orchestration. I list those without apology, because they're interface decisions rather than admissions.

The product bet is that LLM systems need a reliable execution substrate between orchestration and provider calls: a declared graph in, typed artifacts out, inspect before execution, trace after execution, exit code at the boundary. If the surrounding system is a queue worker, `llmff` is the subprocess it supervises. If it's an agent host, `llmff` is the tool that runs a bounded inference pipeline. If it's CI, `llmff` is the command that turns explicit inputs into checked artifacts. Nobody has to give up their planner, memory, scheduler, or product policy for any of that to be useful.

## Try This

Start at the boundary:

```bash
llmff inspect examples/json-repair.yaml --format json
llmff run examples/json-repair.yaml --run-dir .llmff/runs/json-repair-demo
ls .llmff/runs/json-repair-demo
```

Look for the split: the preflight contract in `inspect.json`, live lifecycle metadata in `events.jsonl`, execution evidence in `trace.jsonl`, resume state in `checkpoint.json`, the final process record in `result.json`, and payloads only where the manifest writes them.

That split is the whole argument. The boundary is the product because the boundary is what lets other systems trust the runner without becoming it.
