# Image Brief

Concept image for Article 3: a clean type-state diagram for `llmff` stage outputs.

Show a single stage box labeled `validate_json` receiving `Value::Json`. From it, three status paths branch:

- `Success(Json)` for schema-conforming output.
- `Invalid { value, errors }` for structured output that failed JSON Schema.
- `Skipped` for a guarded stage whose `when` condition did not match.

Place `StageExecutionError` outside the status box, as a separate red or gray failure path for missing files, invalid schema documents, backend errors, and timeouts. The visual should teach the core distinction: semantic invalidity is a workflow state; execution failure is a run failure.

Style: restrained systems diagram, high contrast, no decorative gradients, no mascot, no hype text.
