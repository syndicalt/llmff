# Tool Stage Design

## Goal

Add a production `tool` stage that lets an `llmff` manifest call an explicit external command or HTTP endpoint as part of the pipeline graph.

## Scope

This stage is for deterministic pipeline composition, not autonomous tool discovery. A manifest author must declare exactly what is called.

## Manifest Shape

```yaml
graph:
  - id: call_formatter
    op: tool
    from: render_prompt
    command: ["/bin/cat"]
```

```yaml
graph:
  - id: call_endpoint
    op: tool
    from: render_prompt
    method: POST
    url: http://127.0.0.1:8080/process
    headers:
      content-type: application/json
```

## Semantics

- `from` is required.
- Exactly one tool transport is required:
  - `command` for local process execution.
  - `url` for HTTP execution.
- `command` must be a non-empty argv list. The engine must not execute through a shell.
- Command tools receive the serialized parent value on stdin and return stdout as `Value::Text`.
- Relative command paths containing a path separator are resolved relative to the manifest cwd. Plain executable names continue to use process `PATH`.
- Non-zero command exit is a stage execution error containing exit code and stderr.
- HTTP tools require `method` and `url`.
- HTTP `POST`, `PUT`, and `PATCH` send the serialized parent value as the request body. `GET` and `DELETE` send no body.
- HTTP response text becomes `Value::Text`.
- Non-success HTTP status is a stage execution error containing the status and response body.

## Non-Goals

- No shell command strings.
- No mandatory environment variables.
- No secret interpolation.
- No retries, timeouts, streaming tool output, or structured response decoding in this slice.

## Tests

- Manifest parsing for `command`, `method`, `url`, and `headers`.
- Command tool echoes parent text through stdin/stdout.
- Command tool reports non-zero exits.
- HTTP tool sends the parent body and captures the response body.
- Tool stage rejects missing transport and ambiguous command-plus-url definitions.
