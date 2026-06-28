---
title: "The Execution Layer LLM Systems Were Missing"
date: "2026-06-11"
source: "X"
url: "https://x.com/corelumen/status/2064805136149455055"
tags: ["CoreLumen", "llmff"]
summary: "The missing abstraction in LLM systems is not another agent loop. It is a bounded execution layer: declared graph in, typed artifacts out, process exit code at the boundary."
---

# The Execution Layer LLM Systems Were Missing

Most LLM systems I've seen start the same way: a few model calls sitting in application code, maybe wrapped in a helper function. It works fine for a while. Then the code grows teeth. A prompt needs a schema, the schema needs repair, and the repair needs retries — but not the same retries you'd use for a flaky HTTP transport, because retrying a semantic failure with the same prompt just burns money. A tool call needs a timeout. A failure needs a reason that something upstream can actually act on. A run needs artifacts that survive after the process exits. Six months in, your application runtime has quietly become a pipeline runner, a retry loop, an event stream, a trace writer, an output manager, and about half of an orchestration layer, and none of those jobs were designed. They accreted.

That shape is genuinely hard to supervise, and I don't think the fix is another agent loop. The missing layer is a bounded execution layer.

`llmff` is built around that idea. The honest comparison is FFmpeg, not an agent framework. A caller hands it a declared graph; `llmff` validates the graph, executes the stages, writes the declared outputs and run metadata, and exits with a code the caller can act on. The caller owns why. `llmff` owns what ran.

I keep coming back to that line because it's what keeps the system honest. The application — or the agent host, the queue worker, the human-operated script, whoever is in charge — owns intent. It decides what job exists, what memory should be consulted, what policy applies, which manifest to run, what budget is acceptable, and what happens after the result comes back. What `llmff` owns is one bounded run:

```text
manifest + inputs
  -> inspectable graph
  -> stage execution
  -> declared outputs
  -> trace/events/checkpoint/result
  -> process exit code
```

That is deliberately narrower than an agent platform. It doesn't plan the task, remember prior conversations, choose tools from a product catalog, approve actions, or schedule work across a fleet. Those are all real jobs that need real owners — they just belong above the runner, in the layer that knows why the work exists in the first place.

## A Small Manifest

A manifest is an execution contract, not a suggestion to a framework callback.

```yaml
version: 1
inputs:
  prompt:
    path: question.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: load_prompt
    model: mock:good
outputs:
  final:
    from: draft
    path: answer.txt
```

This says less than a framework object and more than a prompt string, which turns out to be a useful amount to say. There's an input named `prompt`, loaded from `question.txt`. The `draft` stage depends on `load_prompt`. The model alias is `mock:good`. The final output comes from `draft` and lands in `answer.txt`. Nothing here is implicit, which means nothing here has to be reverse-engineered later.

Because the contract is declared, a caller can ask what a run means before spending a single provider request:

```bash
llmff inspect pipeline.yaml --format json
```

The inspect report includes the manifest hash, inputs, outputs, stage order, backend registrations, stdout ownership, execution settings, plugin metadata, and schema compatibility fields. A supervisor can store the whole thing next to the job record. For the manifest above, the important part is plain:

```json
{
  "stage_order": ["load_prompt", "draft"],
  "outputs": {
    "final": {
      "from": "draft",
      "path": "answer.txt"
    }
  },
  "execution": {
    "scheduler": "sequential",
    "stdout": {
      "events": false,
      "manifest_outputs": false,
      "stream_stage": false
    }
  }
}
```

Before anything executes, you know what will run, which files are owned, and whether stdout is safe for machine output. Then the caller runs the bounded job:

```bash
llmff run --run-dir .llmff/runs/job-42 pipeline.yaml
```

The process completes or it fails, and the supervisor learns which from the exit code — no in-process callback API to integrate, no framework object to interrogate.

## Why Subprocess Semantics Matter

A process boundary looks boring until the first incident.

When the LLM framework lives in-process, application code ends up having to know too much. Was this failure a manifest problem, a provider timeout, bad JSON, a local file error, or an interruption? Did a prompt payload leak to stdout? Were any events written before the crash? Which model aliases were configured, and which output path was supposed to exist? I've debugged that kind of incident by grepping application logs for framework internals, and I don't recommend it.

All of those questions get easier when execution is a child process with file-backed artifacts. `llmff run --run-dir <dir>` writes a run-scoped bundle:

```text
.llmff/runs/job-42/
  inspect.json
  events.jsonl
  trace.jsonl
  checkpoint.json
  result.json
```

Each file has a different job, and the differences matter. `inspect.json` is the preflight contract — it describes what should run, which is not the same as proof that it ran. `events.jsonl` is the live supervision stream, the thing a harness tails while the subprocess is still active. `trace.jsonl` is execution evidence for afterwards: debugging, metrics, summaries, compatibility checks. `checkpoint.json` is resume state, bound to the manifest hash so it can't be silently reused after the graph changes. And `result.json` is the final run record — status, exit code, failure kind, retry recommendation, artifact paths — without turning metadata into a prompt dump.

With that bundle on disk, the supervisor's own policy can stay simple:

```text
inspect -> spawn subprocess -> watch events -> wait for exit -> collect artifacts -> decide next step
```

That sequence composes with almost any host — a CLI script, a queue worker, a CI job, an agent harness, a product backend — because every host already knows how to spawn a process and read files. It also gives cancellation and timeouts a clean home. The supervisor can kill the process. `llmff` records an interrupted result when it gets the chance. The caller preserves exit code `130` and decides whether a matching checkpoint makes resume acceptable. No special runtime theology required.

## Exit Codes Beat Vibes

LLM workflows need good traces, but traces shouldn't be the final authority on whether a run succeeded. The process exit code is the hard result; the files add context.

The harness-facing exit codes separate failure classes that genuinely call for different responses:

```text
0    success
2    invalid CLI invocation
10   manifest, graph, config, checkpoint, or static validation failure
20   stage execution failure
21   backend, provider, HTTP tool, or timeout failure
22   local I/O or JSON processing failure
30   intentionally not implemented
130  interrupted
```

The distinction lets a supervisor behave differently without guessing from log text. A `graph_validation` failure should never be retried unchanged — the manifest is wrong, and running it again will produce the same wrongness at the same cost. A provider timeout is a different animal: host policy might retry it, switch backends, or extend the timeout. An interrupted run might be resumable, but only if the manifest hash still matches the checkpoint.

This is where a bounded runner earns its keep. It doesn't make the policy decision; it returns enough structured evidence for whoever owns policy to make it well.

## Artifacts Beat Callbacks

Callbacks are convenient inside one process. They stop being convenient the moment the caller is a queue worker, a shell script, a remote agent harness, or a CI job — which describes most of the places LLM pipelines actually run in production.

Files are less fashionable and more durable. A declared output path can be archived. A trace can be diffed. An inspect report can be stored before execution and compared against reality after. A result record can be read by a supervisor written in a different language. A manifest hash can be checked against a checkpoint without importing the runner as a library.

This is the substance of the FFmpeg analogy. FFmpeg doesn't need to own your video product. It takes declared inputs and flags, performs finite work, writes outputs, and returns a process result — and the calling application remains the product. `llmff` follows the same shape for LLM pipelines: inputs on disk, manifest in version control, providers and tools below, supervisor above, runner in the middle.

There's an aesthetic argument for this shape, but the operational argument is stronger. Boring boundaries make systems easier to restart, inspect, wrap, and replace, and you find out how much that's worth at two in the morning, not during the demo.

## What The Runner Does Not Own

The boundary isn't a disclaimer tacked onto the README. It's a design constraint, and it cuts in specific places.

`llmff` doesn't decide that a user request should become a support-ticket triage job — the application decides that. It doesn't retrieve long-term memory or choose which facts matter; a memory layer decides that and expresses the decision as an input file or a manifest choice. It doesn't decide which tool is allowed for a customer account, because tool policy is a product question and product questions belong in the product. And it doesn't choose the next task after a run finishes. A supervisor reads the exit code, the result record, the trace, and the outputs, and then decides.

I care about this refusal because it's what keeps the runner testable, and it keeps the integration surface small enough to supervise from outside the process. The runner should not know why the job exists. It should know exactly what ran.

## Try This

Start with a mock-backed example, so inspection doesn't require provider credentials:

```bash
llmff inspect examples/templates/summarization.yaml --format json
```

Look at these fields first:

```text
manifest.hash
inputs
outputs
stage_order
execution.stdout
backends.registrations
```

Then run it as a supervised subprocess and look at what it leaves behind:

```bash
llmff run --run-dir .llmff/runs/summarization-1 examples/templates/summarization.yaml
ls .llmff/runs/summarization-1
```

The useful question was never whether `llmff` can grow into the whole agent stack. It shouldn't. The useful question is whether a supervisor can trust it to execute one declared graph, leave typed evidence behind, and stop.

That's the missing execution layer.
