# Map, Batch, And The Math Of Bounded Fan-Out

[IMAGE PLACEHOLDER: Side-by-side technical diagram. Left side: one llmff run containing a map stage over items 0, 1, and 2. Right side: batch mode spawning three isolated manifest runs with separate item output folders and one batch report.]

Fan-out is where LLM pipeline code often loses its shape. One ticket becomes fifty tickets, one extraction becomes a list of extractions, one evaluation prompt becomes a small corpus. The obvious move is to write a loop in the host language, call the model repeatedly, append results into an array, and bolt on some logging afterwards — and it works, right up until the supervisor needs to ask basic questions. How many items can this run touch? Which item produced this output? Did parallel completion reorder the results? If item 17 fails, is that a stage failure inside one run or one failed job inside a batch? Where, exactly, are the artifacts?

`llmff` treats those as execution-contract questions, not comments in application code, and it answers them with two distinct fan-out shapes. `op: map` is in-pipeline collection fan-out inside one bounded run — a finite collection transform. CLI batch mode is job-level fan-out: one manifest run per input line, with isolated item artifacts. The difference between them matters more than either feature on its own, because deterministic fan-out isn't a throughput trick. It's how a supervisor keeps count.

## Map Is Inside The Graph

A map stage takes one parent value, selects a JSON array from it, and runs a small body graph once per item, with the body receiving a reserved value named `item`. Here's a real example shape:

```yaml
version: 1
inputs:
  payload:
    path: ./map-items.json
    format: json
graph:
  - id: load_payload
    op: load
    input: payload

  - id: names
    op: map
    from: load_payload
    items_from: items
    max_items: 3
    final:
      from: name
      require_status: success
    body:
      - id: name
        op: extract
        from: item
        field: name
outputs:
  final:
    from: names
    path: ./map-batch-items.output.json
```

Run the preflight:

```bash
llmff inspect examples/loops/map-batch-items.yaml --format json
```

The inspect report includes the map contract:

```json
{
  "id": "names",
  "op": "map",
  "map": {
    "items_from": "items",
    "max_items": 3,
    "body_stage_count": 1,
    "max_expanded_stage_count": 3,
    "parallel": false,
    "max_concurrency": null
  }
}
```

That report is useful before any item runs: the supervisor sees the item source, the cap, the body size, and the maximum expanded stage count. The static bound is simple:

```text
map_work <= min(len(items), max_items) * body_stage_count
```

There's an honest asymmetry in that formula. `len(items)` is only knowable after the parent value exists at runtime, but `max_items` and `body_stage_count` are in the manifest, so inspect can report a preflight upper bound of `max_items * body_stage_count` — here, `3 * 1 = 3`. That number is not a latency promise or a provider cost guarantee. It's a static execution bound: whatever the data turns out to be, the body graph cannot expand beyond three item executions.

## The Reserved `item` Value

Inside a map body, `item` is the current array entry — not a global variable, not hidden memory, just a reserved body input supplied by the map controller. If the parent JSON looks like this:

```json
{
  "items": [
    { "name": "Ada" },
    { "name": "Grace" },
    { "name": "Katherine" }
  ]
}
```

then each body execution sees one object:

```yaml
body:
  - id: name
    op: extract
    from: item
    field: name
```

The body is still a graph in its own right. It can contain multiple stages — extract, template, infer, validate, score, select — and all of it stays bounded by the item cap. You don't get unbounded autonomy by smuggling a loop into a stage. You get a declared collection transform, which is the point.

## Parallel Map Changes Scheduling, Not Semantics

Map is sequential by default; when parallelism is worth having, the manifest has to say so:

```yaml
- id: map_names
  op: map
  from: load_payload
  items_from: items
  max_items: 3
  parallel: true
  max_concurrency: 2
  final:
    from: name
    require_status: success
  body:
    - id: name
      op: extract
      from: item
      field: name
```

`parallel: true` requires `max_concurrency`, and that pairing is deliberate — parallel fan-out without a concurrency cap is a scheduling policy leaking out of the manifest and into luck.

The property worth underlining: output order stays deterministic by item index, not completion time. That distinction looks pedantic until one item is slow. If item 2 finishes before item 0, the output must not silently reshuffle because the scheduler had a good day; supervisors and downstream stages need stable positions. Trace records for map body work carry the context to keep everything attributable:

```json
{
  "event": "stage_finished",
  "stage_id": "names.name",
  "op": "extract",
  "status": "success",
  "map_id": "names",
  "map_index": 1,
  "map_stage_id": "name"
}
```

The join key — map id, item index, body stage id — is enough for a supervisor to explain which declared body stage ran for which item, regardless of when it actually completed.

## Map Output Is One Run Artifact

A map stage produces one stage value: a JSON value with item results and metadata. Simplified:

```json
{
  "items": [
    { "name": "Ada" },
    { "name": "Grace" },
    { "name": "Katherine" }
  ],
  "metadata": {
    "items_run": 3,
    "items_total": 5,
    "stop_reason": "max_items",
    "parallel": true
  }
}
```

This is still one pipeline run — one manifest, one trace, one result, one output path. Map fits when the collection belongs to the same logical pipeline output: extracting names from a JSON payload, classifying sections inside one document, scoring a fixed list of candidate answers, normalizing a bounded list before a downstream select stage.

It fits poorly when the items are really independent jobs in disguise — thousands of queue entries with their own retry policies, per-customer artifact retention, item-level replay across separate workspaces, or any situation where item 17 should be retried tomorrow without touching item 18. That second group is batch work, and it deserves a different contract.

## Batch Is Outside The Graph

CLI batch mode runs the whole manifest once per input line. The manifest must have exactly one input and file-based outputs; during batch execution, `llmff` writes each input line to an isolated per-item input file, rewrites the manifest outputs under the item's output directory, and records a `batch-report.jsonl`. The command shape:

```bash
llmff run examples/real-world/batch-classification.yaml \
  --batch-input examples/real-world/inputs/batch-items.jsonl \
  --batch-output-dir .llmff/batch/classification \
  --timeout-ms 30000
```

The input file is line-based:

```jsonl
{"id":"ticket-100","text":"Trial user asks whether llmff can classify support issues from a file."}
{"id":"ticket-101","text":"Operator reports a provider timeout during a batch run and wants retry guidance."}
{"id":"ticket-102","text":"Maintainer asks whether package-manager publication is support-ready."}
```

Each line becomes one manifest run, and each item gets a folder that can't collide with its neighbors:

```text
.llmff/batch/classification/
  batch-report.jsonl
  inputs/
    000000.txt
    000001.txt
    000002.txt
  items/
    000000/
      outputs/batch-classification.json
    000001/
      outputs/batch-classification.json
    000002/
      outputs/batch-classification.json
```

The exact output filename comes from the manifest output path after `llmff` rewrites it under the item directory. The report carries one JSON object per item:

```jsonl
{"index":0,"status":"succeeded"}
{"index":1,"status":"succeeded"}
{"index":2,"status":"failed","exit_code":21,"failure_kind":"backend","retry_recommendation":"retry_with_backoff"}
```

If any item fails, batch mode keeps the report on disk and exits non-zero after processing the whole batch. The caller preserves the exit code, then uses the report to decide which items to repair or retry. This is a genuinely different contract from map — not one graph stage producing an array, but a supervisor-facing item runner.

## Batch With A Run Directory

For agent hosts and queue workers, batch mode composes with a run directory so lifecycle metadata and item payloads stay separate:

```bash
llmff run pipeline.yaml \
  --run-dir .llmff/runs/job-42 \
  --batch-input .llmff/runs/job-42/items.txt \
  --batch-output-dir .llmff/runs/job-42/batch-output \
  --timeout-ms 30000
```

```text
.llmff/runs/job-42/
  inspect.json
  events.jsonl
  trace.jsonl
  checkpoint.json
  result.json
  batch-output/
    batch-report.jsonl
    items/
```

Run metadata describes lifecycle and status; batch output owns per-item payload artifacts. This split is also why `llmff` never needs to become a queue. The queue owns leasing, scheduling, backoff, dead-letter policy, and human review. `llmff` owns a bounded subprocess invocation and the artifacts it produced.

## Choosing The Shape

Use `op: map` when item fan-out is part of one declared pipeline — when you want a single run, downstream stages consuming the mapped value, per-item trace context, one logical output artifact, and deterministic item order inside a stage value. Use batch mode when each item deserves isolated artifacts — one manifest run per line, a report that can drive item-level retry, item directories that can't overwrite each other, and a caller who decides what happens after partial failure.

The shortest version:

```text
map   = one run, many bounded body executions, one mapped stage value
batch = many runs, one manifest per item, isolated item artifacts
```

Note that parallelism isn't the design axis here, even though it's the first thing people ask about. For map, `parallel: true` and `max_concurrency` belong in the manifest because item fan-out is part of the graph contract. For batch, item-level scheduling belongs above `llmff` — the invocation can still pass execution controls like `--timeout-ms`, `--retry-attempts`, `--parallel`, and `--max-concurrency` down to each per-item run, but the queue, lease, and retry policy stay with the caller. Parallelism changes when work is scheduled. It should never change what output means.

## What `llmff` Does Not Own

`llmff` doesn't decide which customer jobs enter a batch, lease queue items, own global scheduling, set provider budget policy across a tenant, or store long-lived item history. Those are caller responsibilities, and they should stay that way. The caller owns why this collection is being processed; `llmff` owns what ran, how it was bounded, where the artifacts went, and what exit status came back. That's enough surface area for supervisors to do useful work without the runner turning into a platform.

## Try This

Inspect the map fixture, then run it with a trace:

```bash
llmff inspect examples/loops/map-batch-items.yaml --format json

llmff run examples/loops/map-batch-items.yaml \
  --trace /tmp/llmff-map-batch-items.trace.jsonl
```

Then compare it with a batch invocation:

```bash
llmff run examples/real-world/batch-classification.yaml \
  --batch-input examples/real-world/inputs/batch-items.jsonl \
  --batch-output-dir .llmff/batch/classification
```

Map and batch both fan out. They do not mean the same thing — and that's the point.
