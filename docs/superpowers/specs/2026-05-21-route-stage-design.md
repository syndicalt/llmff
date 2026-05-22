# Route Stage Design

## Purpose

Implement the advertised `route` built-in stage so pipelines can choose between already-computed stage outputs based on validation status or a JSON scalar field.

This moves `llmff` closer to the original inference-graph vision without introducing parallel scheduling yet.

## Manifest Shape

Status-based routing:

```yaml
- id: choose_final
  op: route
  from: validate
  on_success: validate
  on_invalid: repair
```

Field-based routing:

```yaml
- id: choose_model_output
  op: route
  from: classify
  field: kind
  cases:
    simple: fast_answer
    hard: strong_answer
  default: fast_answer
```

## Rules

- `from` identifies the condition source.
- Status routing uses the status of `from`.
- `on_success`, `on_invalid`, and `on_skipped` point to stage ids that already appear earlier in the graph.
- Field routing requires the `from` status to be `Success(Value::Json(object))`.
- Field values must be strings, numbers, or booleans. Values are matched as their compact JSON/string representation.
- `cases` maps field values to stage ids that already appear earlier in the graph.
- `default` is used when no case matches.
- The route output is an exact clone of the selected stage status.
- If no route matches, fail with a `StageExecution` error naming the route stage.

## Graph Validation

`Graph::from_manifest` validates route target references:

- `on_success`
- `on_invalid`
- `on_skipped`
- `cases` values
- `default`

Targets must refer to stages that are earlier in manifest order. This keeps the current sequential scheduler honest and avoids hidden forward dependencies.

## Out Of Scope

- Parallel branch execution.
- Predicate expressions.
- Numeric comparison operators.
- Nested JSON field paths.

These are excluded from this slice because the current sequential scheduler cannot execute branches lazily, and the route target model leaves room for those capabilities in a later scheduler upgrade.

## Testing

- Manifest parsing for route fields.
- Graph validation rejecting unknown route targets.
- Engine route selecting success, invalid, skipped, case, and default targets.
- CLI example still inspects and runs.
