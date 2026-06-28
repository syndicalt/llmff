# Inspect Before You Run

[IMAGE PLACEHOLDER: Header diagram showing an `inspect.json` report card beside a bounded pipeline: stage order, model aliases, stdout ownership, loop bounds, and artifact paths.]

The worst time to learn what an LLM pipeline is going to do is after it has opened a provider connection. That sounds obvious, but look at how many systems treat "run" as the first real interface: the caller sends a prompt, a framework builds messages, selects tools, retries, logs, streams, and eventually returns something, and if the caller wants to know what happened it gets to read callbacks or rummage through logs after the fact.

`llmff` takes a different stance. Before the run, ask the runner what it's about to execute:

```bash
llmff inspect examples/loops/self-refining-answer-loop.yaml --format json
```

That command doesn't call a model, doesn't run a tool, and doesn't produce a payload. It turns the manifest into a machine-readable preflight contract. I care about this boundary because a supervisor shouldn't have to trust a pipeline by vibe — it should be able to inspect the graph, decide whether the work fits its budget and policy, and only then dispatch the subprocess. Trust starts before execution.

## What Inspect Is For

`inspect` exists for machines. Humans can read it, but the primary consumer is a supervisor, a CI gate, a release script, a queue worker, or an agent host that needs answers to boring operational questions: which manifest is this, which inputs and outputs does it declare, what stage order will run, which models and backend aliases are referenced, which plugin capabilities are involved, which paths own the trace, event, checkpoint, and output artifacts, does anything write to stdout, and are the loops and maps bounded.

Boring questions, but they're the difference between "please run this pipeline" and "please run this reviewed execution contract." The caller owns why the job exists; `llmff` owns what ran. `inspect` is where those two sides meet, before anything expensive or irreversible happens.

## The Smallest Useful Shape

A typical supervisor pattern looks like this:

```bash
llmff inspect pipeline.yaml --format json > .llmff/runs/job-42/inspect.json

llmff run --run-dir .llmff/runs/job-42 pipeline.yaml
status=$?

exit "$status"
```

With `--run-dir`, the run owns a local artifact bundle:

```text
.llmff/runs/job-42/
  inspect.json
  trace.jsonl
  events.jsonl
  checkpoint.json
  result.json
```

None of this is ceremony for its own sake. The point is that the run gets a before picture and an after picture: `inspect.json` says what was expected to run, `trace.jsonl` and `events.jsonl` say what happened, and `result.json` plus the process exit code say how the subprocess ended. An agent host or queue worker can store those artifacts next to its own job record, compare manifest hashes, check stage order, and make retry or escalation decisions without ever parsing prose logs.

## A Real Inspect Fragment

For the self-refining answer loop, the top-level stage order is short:

```json
{
  "stage_order": [
    "load_question",
    "build_initial_prompt",
    "refine_loop"
  ]
}
```

Three stages doesn't mean three pieces of work — the third stage is a bounded loop, and its inspect metadata carries the upper bound:

```json
{
  "id": "refine_loop",
  "op": "loop",
  "from": "build_initial_prompt",
  "loop": {
    "max_iterations": 5,
    "body_stage_count": 5,
    "max_expanded_stage_count": 25,
    "break_on": {
      "type": "field_true",
      "stage": "quality_ready",
      "field": "passed"
    },
    "final": {
      "from": "final_answer",
      "require_status": "success"
    },
    "retain_iterations": "none",
    "on_iteration_error": "fail"
  }
}
```

That fragment is small, but it carries the operational shape of the work. The loop runs at most five iterations of a five-stage body, so the static upper bound is twenty-five expanded body stages. The break condition is explicit. The final value must come from `final_answer`. Iteration retention is off, so the loop isn't silently preserving every body value as output. The arithmetic is as simple as it looks:

```text
max_expanded_stage_count = body_stage_count * max_iterations
                         = 5 * 5
                         = 25
```

And it's an upper bound, not a prediction. The break condition may stop the loop earlier, a stage may fail, a host timeout may kill the subprocess, a backend may reject a request. What cannot happen is the manifest expanding into a sixth iteration by surprise — and that's the property worth paying for.

## Bounds Are Not Cost Estimates

I want to be precise about what static bounds don't promise, because overclaiming here would undermine the whole argument. Bounds don't know token cost. They don't know prompt size after templating, provider-side behavior, or completion length unless the manifest and backend enforce one. They don't know whether a semantic repair step will need one attempt or several.

What they do tell the supervisor is how large the declared graph can become before runtime data starts affecting control flow. For loops, `max_expanded_stage_count = body_stage_count * max_iterations`; for maps, the same with `max_items`. That's enough for real policy: a queue worker can reject a job whose loop bound exceeds its limits, a CI check can require every loop to declare a maximum, an agent host can show the planned shape to a human approver before dispatch.

Cost control starts with static bounds for an unglamorous reason — they're the only numbers available before the first provider call.

## Inspect Also Names The Backends

Model strings in a manifest aren't enough on their own; a supervisor needs to know how those strings resolve at the runner boundary. So the inspect report makes backend registrations explicit:

```json
{
  "backends": {
    "registrations": [
      {
        "name": "mock",
        "kind": "deterministic",
        "source": "built-in",
        "registration_flag": "built-in",
        "requires_api_key": false,
        "model_aliases": ["mock:bad", "mock:good", "mock:json"]
      },
      {
        "name": "openai-compatible",
        "kind": "remote-chat",
        "source": "built-in",
        "registration_flag": "--backend <alias>=<base-url>",
        "requires_api_key": true,
        "model_aliases": []
      }
    ]
  }
}
```

The report is deliberately plain. The manifest stays portable, while the caller can still see whether a referenced backend is deterministic, local, or remote, whether it needs credentials, and which CLI registration shape is expected.

This is another place where `llmff` declines to own policy and instead exposes the information policy needs. A production worker that only allows local Ollama aliases can inspect and reject remote registrations. A CI suite that requires deterministic runs can verify the manifest only references `mock:*` models. A deployment that requires credentials from a specific environment variable can enforce that outside the manifest entirely. The runner executes the declared graph; the host decides whether that graph is allowed here.

## Stdout Ownership Is A Preflight Concern

Stdout is a small surface with outsized consequences. In a command-line runner, stdout can carry lifecycle events, a streamed stage payload, or a declared manifest output — and the moment it carries more than one of those at once, every machine consumer downstream is parsing mixed text. Inspect reports include stdout ownership so the layout can be settled before execution:

```json
{
  "execution": {
    "stdout": {
      "events": false,
      "stream_stage": false,
      "manifest_outputs": false
    },
    "artifacts": {
      "trace": null,
      "events": null,
      "stream_stage": null
    }
  }
}
```

A supervisor then picks a clean stream layout for its situation. For live events:

```bash
llmff run pipeline.yaml \
  --events - \
  --trace .llmff/runs/job-42/trace.jsonl
```

For a streamed model stage:

```bash
llmff run pipeline.yaml \
  --stream-stage draft \
  --events .llmff/runs/job-42/events.jsonl \
  --trace .llmff/runs/job-42/trace.jsonl
```

For the simplest artifact bundle:

```bash
llmff run --run-dir .llmff/runs/job-42 pipeline.yaml
```

`llmff` rejects conflicting stdout owners outright. That's not a usability flourish — it's what keeps machine protocols from becoming mixed text. If events are JSONL, they stay JSONL. If a payload is a model answer, it stays a payload. If a manifest output writes to `-`, nothing else streams lifecycle metadata there. Small rules like this are most of what makes a subprocess pleasant to supervise.

## Plugin Metadata Belongs In The Same Report

Plugins are another place where hidden runtime behavior gets expensive, so the inspect report includes plugin directories, protocol version, manifests, and capability names:

```json
{
  "plugins": {
    "directories": ["examples/plugins"],
    "protocol_version": 1,
    "manifests": [
      {
        "name": "postprocessor-strip",
        "version": "0.1.0",
        "capabilities": [
          {
            "kind": "stage",
            "name": "postprocessor.strip",
            "entrypoint": "./bin/postprocess"
          }
        ]
      }
    ]
  }
}
```

As with backends, the report doesn't make the policy decision — it gives the supervisor a stable place to make one. A host might allow a checked-in postprocessor while rejecting arbitrary plugin directories, require plugin protocol version `1`, record entrypoints for review, or run `llmff plugins validate` in CI before any worker is allowed to dispatch the manifest. The mechanism underneath stays boring: local executables, declared capabilities, protocol metadata, process semantics.

## What Inspect Does Not Do

`inspect` doesn't prove a provider will return valid JSON, or that a remote model will stay available, or that the output will be any good. It doesn't turn an LLM workflow into a deterministic program. Those would be the wrong promises, and a tool that made them would deserve the skepticism it got.

The promise it does make is narrower and keepable: before execution, a caller can see the declared graph, the static validation result, the stage order, compatibility versions, the model and backend surface, the plugin surface, output ownership, the artifact layout, and every loop or map bound. That's enough to move a surprising number of operational decisions out of the hot path — reject the manifest before a queue slot is consumed, ask for human approval before a remote backend is used, store the manifest hash next to the job record, compare a checkpoint against the same manifest before resuming, keep artifact paths predictable.

These are ordinary systems moves. LLM pipelines just need more of them.

## The Supervisor Pattern

The practical integration sequence is short: inspect, run, preserve the exit code, store the artifacts, decide the next step. In shell:

```bash
set +e

llmff inspect pipeline.yaml --format json > .llmff/runs/job-42/inspect.json
inspect_status=$?
if [ "$inspect_status" -ne 0 ]; then
  exit "$inspect_status"
fi

llmff run --run-dir .llmff/runs/job-42 pipeline.yaml
run_status=$?

# The supervisor can read result.json, events.jsonl, and trace.jsonl here.
# It should still preserve the original llmff exit code.
exit "$run_status"
```

In a larger host, the same shape becomes a few method calls: allocate a run dir, write or select a manifest, inspect it, enforce local policy, spawn `llmff run`, stream or store events, wait for process exit, preserve the exit code, read the artifacts, and choose retry, repair, escalation, or success.

Notice what's absent from that list. The host never imports the runner's internals, and the runner never touches the host's memory, planner, queue, permissions, or approval flow. The process boundary is doing real work.

## Why This Matters

LLM systems blur intent and execution all the time, and early on the blur feels convenient — one runtime chooses the prompt, calls the model, retries, routes, logs, streams, and decides what to do next. The cost arrives the day another system has to supervise it, and suddenly nobody can answer the basic questions. What's the bound? Which model did this use? Who owns stdout? Which artifacts are payloads and which are metadata? Did the manifest change since the checkpoint? Was this failure a provider failure, a graph validation failure, or a local I/O problem?

`inspect` doesn't answer every runtime question. It answers the questions that should never have been runtime questions in the first place. An inspect report is a contract between a pipeline author and a runner — and between the runner and the supervisor deciding whether this finite piece of work is allowed to run at all.

The safest LLM call is the one your supervisor inspected before it ran.
