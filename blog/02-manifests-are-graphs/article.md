---
title: "Manifests Are Graphs, Not Prompt Wrappers"
date: "2026-06-12"
source: "X"
url: "https://x.com/compose/articles/edit/2064863835471679488"
tags: ["llmff"]
summary: " manifest is not a config file for a prompt. It is a small typed graph: inputs, stage IDs, dependencies, operations, and outputs."
---

# Manifests Are Graphs, Not Prompt Wrappers

[IMAGE PLACEHOLDER: Minimal DAG diagram showing load_prompt -> build_prompt -> draft -> validate_answer -> write_answer. Each node has its stage ID. A side panel links the same IDs across manifest, trace, and output.]

The easiest mistake to make with `llmff` manifests is to read them as a larger prompt wrapper — a prompt with some parameters arranged around it. That reading misses the useful part.

A prompt wrapper starts with text and decorates it: model, temperature, maybe a schema, maybe a retry count. However much you add, the center of gravity stays in the prompt. A pipeline has a different shape entirely. It has inputs, operations, dependencies, outputs, and failure states. Some stages call models, but others load files, validate JSON, route around invalid output, or write artifacts, and the interesting structure lives in how those stages connect. The useful unit isn't the model call. It's the declared run.

In `llmff`, the manifest is where intent becomes an execution contract — and that contract is a graph.

## The Smallest Useful Shape

Here's a compact manifest:

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

There are two stages, `load_prompt` and `draft`, and each is a vertex. The `from: load_prompt` field is an edge. The output declaration is an edge too:

```yaml
outputs:
  final:
    from: draft
    path: answer.txt
```

That last edge tells the runner which computed value becomes the declared output artifact. It's a small amount of structure, but it's already enough for `llmff inspect` to answer concrete questions before anything executes:

```bash
llmff inspect pipeline.yaml --format json
```

```json
{
  "stage_order": ["load_prompt", "draft"],
  "outputs": {
    "final": {
      "from": "draft",
      "path": "answer.txt"
    }
  }
}
```

The stage IDs deserve more credit than they usually get. They're not decoration — they're stable handles that connect the manifest to the inspect report, trace events back to the manifest, and supervisor decisions to a specific operation. They also give humans a vocabulary for debugging that beats "the second model call after the loader." With two nodes that hardly matters. By the time a graph has a validation step, a repair path, and a route, stable names are the difference between reading a trace and decoding one.

## A Graph, Not A Script

Manifests are usually written in a readable top-to-bottom order, but the runtime cares about dependencies, not line numbers. This manifest is deliberately listed out of order:

```yaml
version: 1
inputs:
  prompt:
    path: question.txt
graph:
  - id: draft
    op: infer
    from: load_prompt
    model: mock:good
  - id: load_prompt
    op: load
    input: prompt
outputs:
  final:
    from: draft
    path: answer.txt
```

The dependency still says `draft` needs `load_prompt`, so `llmff inspect` normalizes it into execution order:

```json
{
  "stage_order": ["load_prompt", "draft"]
}
```

That's graph behavior. A linear script either runs top to bottom or makes the author responsible for keeping every statement manually ordered forever. A graph runtime can validate references across the whole stage set, sort by dependency, and reject impossible structures before the run starts. The distinction feels minor at two stages. Once the workflow grows a repair path, a route, a loop body, or a map body, it becomes the difference between an inspectable execution contract and a pile of conventions that happen to work.

## The DAG Model

The formal model is small enough to state in one line:

```text
G = (V, E)
```

`V` is the set of stages; `E` is the set of data dependencies. In a basic manifest, a `from` field creates an edge (`load_prompt -> draft`), and outputs create edges from stage values to artifact names (`draft -> outputs.final`). Other manifest features create edges too: route targets create dependencies, loop and map bodies carry their own scoped body graphs, and body references connect stages inside those bodies. The features vary, but the discipline doesn't — if one operation needs the value of another, the manifest says so.

A valid execution graph has a topological order: every stage appears after the stages it depends on. If no such order exists, the graph is wrong, full stop. This structure, for instance, cannot be executed:

```yaml
graph:
  - id: a
    op: infer
    from: b
    model: mock:good
  - id: b
    op: infer
    from: a
    model: mock:good
```

`a` needs `b` and `b` needs `a`, so there's no first stage. The correct behavior is to reject that before any provider call happens — a cycle isn't a runtime surprise, it's a graph validation failure, and it should cost zero tokens to discover. Missing references belong in the same category:

```yaml
graph:
  - id: draft
    op: infer
    from: missing_loader
    model: mock:good
```

If `missing_loader` isn't an input or a stage value the graph can resolve, the manifest was never a runnable contract to begin with. This is why "inspect before you run" is more than a slogan: the structural failures are exactly the ones the runner can catch while the cost is still zero model calls.

## Typed Operations Make The Graph Useful

A graph of arbitrary strings would still be too loose to validate. What makes the structure pay off is that each stage operation carries a contract: `load` reads a declared input, `infer` consumes a parent value and a model alias, `validate_json` consumes text or JSON and checks it against a schema, and outputs consume stage values and write artifacts.

A slightly richer manifest shows why that matters:

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
    response_format: json
  - id: validate_answer
    op: validate_json
    from: draft
    schema: '{"type":"object","required":["answer"],"properties":{"answer":{"type":"string"}},"additionalProperties":false}'
outputs:
  final:
    from: validate_answer
    path: answer.json
```

The graph is `load_prompt -> draft -> validate_answer -> outputs.final`, and now the runtime can inspect more than ordering. It can check that `validate_json` has a `from` source and either an inline `schema` or a schema path. It can check that `draft` names a configured model alias. It can report that the output artifact is bound to `validate_answer`. None of that is possible when the pipeline is a prompt string with comments around it.

## Reproducibility Lives At Run Scope

It's tempting to treat the model call as the unit of reproducibility. It's too small.

A model response depends on the loaded input, the stage that constructed or selected the parent value, the model alias, the sampling fields, the validation schema, the repair or routing path, the output declaration, and the runtime options. Change any one of those and the run changed, whether or not the prompt text did. That's why `llmff inspect --format json` reports a manifest hash tied to the declared run rather than to any single provider request.

Before execution, a supervisor can store the manifest hash, stage order, input paths, output paths, backend registrations, execution options, and schema versions. After execution, it stores the exit code, `result.json`, `events.jsonl`, `trace.jsonl`, and the declared outputs. The link between the two sets is the manifest and its stable stage IDs, which gives you a practical debugging path: which run failed, which manifest hash did it use, which stage ID failed, what operation was that stage, what did the inspect report say should happen, and what did the trace say actually happened.

Those are better questions than "which prompt did we send?"

## What The Manifest Does Not Own

A manifest declares execution. The temptation, as it grows, is to let it become a hidden planning language — and that temptation is worth resisting explicitly.

The manifest doesn't decide the user goal, choose long-term memory, decide whether a customer account may use a tool, schedule future jobs, or pick the next manifest after the run. Those choices belong to the supervisor, which has the context to make them. The manifest's job is narrower: say what finite work will run if selected.

That restraint keeps the graph small enough to inspect and concrete enough to operate. It also makes room for different hosts — a product backend, an agent harness, a data pipeline, and a local CLI script can all choose the same manifest for entirely different reasons, and the runner never needs to know those reasons. It needs the declared dependencies.

## Try This

Inspect the small mock-backed template:

```bash
llmff inspect examples/templates/classification.yaml --format json
```

Look for:

```text
stage_order
stages[].id
stages[].op
stages[].from
outputs.final.from
manifest.hash
```

Then compare the YAML stage IDs to the JSON report. The same IDs carry through the system — manifest, inspection, execution, tracing, outputs, supervision — and that continuity is the point. The manifest is not a prompt wrapper. It's a small typed graph with names that survive the whole run.
