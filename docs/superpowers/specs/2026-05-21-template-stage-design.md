# Template Stage Design

## Purpose

Implement the advertised `template` built-in stage so manifests can compose prompts from version-controlled template files.

This closes a correctness gap: `llmff stages list` advertises `template`, and the original design names template as an initial built-in stage, but the engine currently rejects `op: template`.

## Behavior

`template` reads template text and substitutes `{{name}}` placeholders.

```yaml
- id: render_prompt
  op: template
  from: load_prompt
  path: ./prompt.tmpl
```

Rules:

- `path` is required for this slice.
- Relative paths resolve against the manifest directory through the engine `cwd`.
- If the parent value is `Text`, one variable is available: `input`.
- If the parent value is `Json` object, each object field is available by its key.
- JSON string fields render as their string value.
- JSON number, boolean, array, and object fields render as compact JSON.
- Missing variables fail with a `StageExecution` error naming the placeholder and stage id.
- Template output is `Value::Text`.

Out of scope:

- Conditionals or loops.
- Escaping rules beyond simple `{{name}}`.
- Inline template strings.
- Loading multiple template files.

## Example

```text
User request:
{{input}}

Return only valid JSON.
```

The example manifest can insert a `template` stage between `load` and `system`, proving the stage participates in normal inference pipelines.

## Testing

- Unit test for text parent substitution via `{{input}}`.
- Unit test for JSON object field substitution.
- Unit test for missing variable errors.
- Engine or CLI test proving `template` runs inside a manifest.
- Example run still produces `{"answer":"ok"}`.
