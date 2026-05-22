# Inline Graph Design

## Goal

Add the first production slice of FFmpeg-like inline graph execution:

```bash
llmff run -i prompt.txt -g 'load | infer(model=mock:good) | write(-)'
```

This keeps manifests as the reproducible format while giving the CLI a compact shell-native graph form.

## Scope

The inline graph parser supports a linear pipeline in this slice. Branching remains manifest-only until the graph DSL has labels.

Supported stage syntax:

```text
op
op(value)
op(key=value,key2=value2)
```

Supported initial operations:

- `load`
- `system(path)`
- `template(path)`
- `infer(model=...,temperature=...)`
- `validate_json(schema_path=...)`
- `repair(model=...,temperature=...)`
- `write(path)`

## CLI Shape

- `llmff run <manifest>` remains supported.
- `llmff run -g <graph>` runs an inline graph.
- `llmff run -i <path> -g <graph>` supplies the default `prompt` input path.
- Without `-i`, inline `load` reads stdin.
- `--trace`, `--backend`, `--api-key-env`, and `--api-key` continue to work for both manifest and inline graph runs.
- Exactly one of `<manifest>` or `-g/--graph` is required.

## Normalization

The parser converts inline syntax into the existing `Manifest` data model:

- `version: 1`
- One input named `prompt` when `load` is present.
- Deterministic stage ids from operation names and ordinal positions, such as `load_1`, `infer_2`, `write_3`.
- Each stage after `load` uses `from` pointing at the previous stage id.
- `write(path)` becomes `op: write` with `path`.
- Inline graphs rely on `write` stages for output. They do not need top-level `outputs`.

## Parsing Rules

- The pipe character separates stages.
- Empty stages are invalid.
- Unknown operations are accepted by the parser and rejected by graph/engine validation, matching manifest behavior.
- Parameters are comma-separated `key=value` pairs.
- Positional values are supported only for path-like operations named above.
- Quoting and escaping are intentionally not included in this slice. Paths and model ids should not contain `|`, `,`, `(`, `)`, or `=`.

## Tests

- Core parser converts `load | infer(model=mock:good) | write(-)` into a valid manifest.
- Core parser converts path-like positional syntax for `template` and `write`.
- CLI runs an inline graph with `-i`, mock backend, and `write`.
- CLI rejects commands that provide both manifest and `--graph`.
