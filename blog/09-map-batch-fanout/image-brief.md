# Image Brief

Create a clean technical diagram for Article 9, "Map, Batch, And The Math Of Bounded Fan-Out."

Concept: side-by-side comparison of map and batch.

Left side: one large box labeled "one llmff run" containing a pipeline graph. Inside the graph, show `load_payload -> names (op: map) -> final`. The `names` map stage fans out internally to item 0, item 1, item 2, each running the same small body graph `item -> name`. The fan-in returns to one mapped stage value. Add labels: `max_items: 3`, `max_expanded_stage_count: 3`, `output order by item index`.

Right side: three separate boxes labeled `run 000000`, `run 000001`, `run 000002`, all created from the same manifest. Each has its own input file and output folder under `batch-output/items/<index>/`. A single `batch-report.jsonl` sits above or below them. Add label: "job-level fan-out, isolated artifacts."

Style: neutral, precise, educational. White or very light background. Thin lines. No mascots, no decorative gradients, no marketing copy. Use exact labels from the article where possible.
