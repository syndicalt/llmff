# Image Brief

Create a technical header image for "Traces, Events, And Observability Without Prompt Logging."

Concept: a JSONL trace timeline that shows metadata records flowing into a supervisor dashboard, while prompt and response payloads remain in separate declared artifact boxes.

Visual direction: precise systems diagram. Use clear labels and small JSON fragments. Avoid surveillance/security panic visuals, locks as the dominant metaphor, or decorative AI imagery. The image should communicate disciplined observability rather than secrecy theater.

Suggested layout:

- Left: `llmff run` process emitting two streams: `events.jsonl` and `trace.jsonl`.
- Center: timeline rows for `run_started`, `stage_started`, `stage_finished`, `run_failed`.
- Include a nested loop row with `loop_id`, `loop_iteration`, and duration bar.
- Right: dashboard/metrics panel showing stage duration, token total, and failure kind.
- Bottom/right separate box: declared payload artifacts, visually distinct from metadata streams.

