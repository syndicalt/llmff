# Parallel Scheduler Design

## Goal

Add an opt-in parallel scheduler for independent ready stages so branching inference graphs can run fan-out model calls concurrently without changing the manifest format.

## Rationale

The original pipeline-runner design says the MVP scheduler may be sequential, but the graph model should support fan-out and fan-in so parallel execution can be added later. `llmff` already validates and orders graph dependencies, but it still executes one stage at a time. That underuses backends for independent branches and weakens the FFmpeg-like graph-runner shape.

## Behavior

- Default execution remains sequential.
- `RunOptions` gains a scheduler mode with `Sequential` and `Parallel`.
- Parallel mode executes all currently ready stages concurrently.
- A ready stage is one whose graph dependencies already have statuses.
- A stage dependency includes:
  - `from`
  - route status targets: `on_success`, `on_invalid`, `on_skipped`
  - route field targets: `cases` and `default`
- Each ready batch uses a consistent snapshot of completed statuses.
- Trace output remains deterministic:
  - write `stage_started` events for a ready batch in graph order before running that batch
  - write `stage_finished` events in graph order after the batch completes
- Top-level outputs are written only after all graph stages finish.
- The CLI exposes this through `llmff run --parallel`.

## Non-Goals

- No worker pool sizing yet.
- No speculative execution of route targets beyond already-required graph dependencies.
- No concurrent trace writes.
- No change to manifest syntax.
- No attempt to parallelize blocking local command execution beyond the async scheduler boundary.

## Verification

- Core test proves default scheduler runs independent async model stages sequentially.
- Core test proves parallel scheduler overlaps independent async model stages.
- CLI test proves `llmff run --parallel` is accepted and still executes a pipeline.
- Full workspace tests and example inspect pass.
