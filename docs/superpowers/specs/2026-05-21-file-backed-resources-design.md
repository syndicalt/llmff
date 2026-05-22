# File-Backed Resources Design

## Purpose

Make manifests more usable as reproducible recipes by allowing stages to load schema and system prompt content from files relative to the manifest directory.

This keeps the FFmpeg-like command shape: the manifest names concrete inputs and transformations, while larger resources live in normal files that can be version controlled.

## Scope

In scope:

- Add `schema_path` to `validate_json`.
- Keep inline `schema` working for compatibility.
- Make `system path: ./policy.md` read text and prepend it to the parent prompt.
- Resolve `schema_path` and `system path` relative to the manifest directory.
- Update the example manifest to use `examples/answer.schema.json`.
- Add tests for relative paths, missing files, and inline schema compatibility.

Out of scope:

- Chat message role modeling.
- Prompt templating.
- Multiple system messages.
- Remote resource URLs.
- Resource caching.

## Behavior

`validate_json` accepts either inline schema or file-backed schema:

```yaml
- id: validate
  op: validate_json
  from: draft
  schema_path: ./answer.schema.json
```

Rules:

- `schema_path` is read as UTF-8 text.
- Relative paths resolve against the `cwd` passed to the engine, which the CLI sets to the manifest directory.
- If both `schema` and `schema_path` are present, inline `schema` wins. This preserves current behavior and avoids surprising users who add a file path while experimenting.
- If neither is present, the stage fails with a clear error.

`system` accepts optional file-backed text:

```yaml
- id: apply_policy
  op: system
  from: load_prompt
  path: ./policy.md
```

Rules:

- If `path` is present, read it as UTF-8 text relative to the manifest directory.
- If the parent value is text, output `system_text + "\n\n" + parent_text`.
- If `path` is absent, preserve existing pass-through behavior.
- If the parent value is JSON, convert it to compact JSON text before appending.

## Errors

Missing or unreadable resource files return `StageExecution` errors naming the stage id and path.

Invalid schema JSON continues to return a stage execution error.

## Testing

Tests should remain deterministic and local:

- Unit test for `validate_json` loading `schema_path` relative to a temp directory.
- Unit test for inline `schema` still working.
- Unit test for missing `schema_path` returning a clear path error.
- Unit test for `system path` prepending file text to parent text.
- CLI or engine integration test using the example manifest with `answer.schema.json`.
