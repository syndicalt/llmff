# Write Stage Design

## Goal

Implement the advertised `write` stage so graph pipelines can persist an intermediate or final value as a normal stage operation.

## Manifest Shape

```yaml
graph:
  - id: save_answer
    op: write
    from: validate
    path: ./answer.json
```

## Semantics

- `from` is required.
- `path` is required.
- `path: -` writes the serialized value to stdout.
- File paths are resolved relative to the manifest directory.
- The parent value must be `StageStatus::Success`.
- Text values are written as-is.
- JSON values are written as compact JSON.
- Invalid or skipped parent statuses are stage execution errors.
- `write` returns the same successful value it wrote, so downstream stages or top-level `outputs` can still reference it.

## Non-Goals

- No append mode.
- No directory creation.
- No streaming writes.
- No format conversion beyond the existing text/JSON serialization.

## Tests

- `write` writes a parent value to a file and forwards that value.
- `write` rejects missing `path` during graph validation.
- Existing `outputs` behavior remains supported.
