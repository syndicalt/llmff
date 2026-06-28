# Image Brief

Create a clean technical header image for "Inspect Before You Run."

Concept: an `inspect.json` report card sitting between a supervisor process and an `llmff` pipeline graph. The report card should call out five fields: stage order, model aliases, plugin metadata, stdout ownership, and loop bounds.

Visual direction: diagrammatic, not decorative. Use a restrained systems palette with high contrast text blocks and thin connector lines. The central idea should be preflight inspection before provider/tool execution. Avoid futuristic AI imagery, glowing brains, robots, or marketing-style abstraction.

Suggested layout:

- Left: "Supervisor" box with policy checks.
- Center: `llmff inspect --format json` report card.
- Right: small DAG with a bounded loop node labeled `max_iterations: 5`.
- Bottom: artifact row showing `inspect.json`, `trace.jsonl`, `events.jsonl`, `result.json`.

