# Companion Posts

1. Map is in-pipeline fan-out. Batch is job-level fan-out.

`op: map` runs a bounded body graph inside one manifest run. CLI batch mode runs the whole manifest once per input line and writes isolated item artifacts.

Same family of problem. Different execution contract.

2. Parallel map should not make output order nondeterministic.

In `llmff`, `parallel: true` requires `max_concurrency`, and mapped outputs stay ordered by input index instead of completion time.

Scheduling can change. Semantics should not.

3. A bounded map is a finite collection transform, not a scheduler.

The useful preflight math is simple:

`max_expanded_stage_count = max_items * body_stage_count`

That tells a supervisor how large the declared body expansion can get before any provider call runs.

4. Batch mode is for artifact isolation.

Each input line becomes its own manifest run, and outputs are rewritten under `batch-output/items/<index>/`.

If one item fails, the batch report survives and the caller decides what to retry.

5. The clean distinction:

`map = one run, many bounded body executions, one mapped stage value`

`batch = many runs, one manifest per item, isolated item artifacts`

Fan-out is not one feature. It is two different boundaries.
