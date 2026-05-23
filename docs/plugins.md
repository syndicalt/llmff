# Plugin Author Guide

llmff plugins are command-line programs declared by `llmff-plugin.yaml` files. A plugin directory contains one or more child plugin roots, and each plugin root contains one manifest:

```text
plugins/
  my-plugin/
    llmff-plugin.yaml
    bin/my-command
```

Use plugins with `--plugin-dir plugins`. Validate manifests and entrypoints before running a pipeline:

```sh
llmff plugins validate --plugin-dir examples/plugins
llmff plugins list --plugin-dir examples/plugins
```

## Manifest Schema

`llmff-plugin.yaml` is YAML with these fields:

```yaml
name: my-plugin
version: 0.1.0
capabilities:
  - kind: stage
    name: text.uppercase
    entrypoint: ./bin/uppercase
```

Fields:

- `name`: non-empty plugin package name.
- `version`: non-empty plugin package version.
- `capabilities`: non-empty list of commands exposed by the plugin.
- `capabilities[].kind`: one of `stage`, `backend`, `sampler`, or `tool-transport`.
- `capabilities[].name`: capability name used by pipelines or CLI discovery.
- `capabilities[].entrypoint`: command path. Relative paths resolve from the plugin root containing `llmff-plugin.yaml`; absolute paths are used as written.

## Command Protocol

Each capability is a separate process. llmff starts the entrypoint, writes one request to stdin, closes stdin, waits for exit, and reads stdout. A non-zero exit status fails the stage or backend call, and stderr is included in the user-facing error.

Stage plugins:

- Manifest kind: `stage`
- Pipeline op: `plugin:<capability-name>`
- stdin: parent stage text
- stdout: replacement stage text

Tool transport plugins:

- Manifest kind: `tool-transport`
- Pipeline use: `op: tool` with `transport: <capability-name>`
- stdin: parent stage text
- stdout: tool response text

Backend plugins:

- Manifest kind: `backend`
- Pipeline model: `<capability-name>:<model-name>`
- stdin: JSON `InferRequest`
- stdout: JSON object with `text` and optional `usage`

Backend response:

```json
{"text":"answer","usage":{"prompt_tokens":1,"completion_tokens":2,"total_tokens":3}}
```

Sampler plugins:

- Manifest kind: `sampler`
- Pipeline use: `sampler: <capability-name>` on `infer` or `repair`
- stdin: JSON `InferRequest`
- stdout: JSON sampling overrides

Sampler response fields are optional. Supported fields are `temperature`, `top_p`, `max_tokens`, `seed`, `response_format`, and `stop`.

```json
{"temperature":0,"max_tokens":64,"seed":7}
```

## Working Directory

Entrypoints are resolved from the plugin root when declared as relative paths.

Stage and tool transport commands run with the pipeline working directory as their current directory, so relative file access inside those commands should be treated as relative to the manifest being run. Backend and sampler commands should not depend on the current directory; use paths relative to the entrypoint or absolute paths if they need local assets.

## JSON Contracts

Backend and sampler plugins should treat stdin as a single JSON document and stdout as a single JSON document. Do not print logs on stdout. Write diagnostics to stderr.

The request includes the model name, chat messages, and sampling parameters:

```json
{
  "model": "echo-backend:test",
  "messages": [{"role": "user", "content": "hello"}],
  "temperature": null,
  "top_p": null,
  "max_tokens": null,
  "seed": null,
  "response_format": null,
  "stop": []
}
```

## Security Boundaries

Plugins are local executables. llmff does not sandbox them. A plugin can read files available to the user, spawn processes, use the network, and write output wherever its OS permissions allow.

Install and run plugins only from trusted sources. Prefer checked-in scripts, pinned dependencies, and reviewable source. Avoid manifests that point at mutable global commands unless that is intentional. Use `llmff plugins validate` to catch malformed manifests and missing entrypoints before a run; it is not a security scanner.

## Examples

See `examples/plugins` for minimal working examples:

- `stage-uppercase`: `stage` capability.
- `backend-echo`: `backend` capability.
- `sampler-small`: `sampler` capability.
- `tool-stdio-cat`: `tool-transport` capability.
