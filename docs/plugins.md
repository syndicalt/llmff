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
llmff plugins validate --plugin-dir examples/plugins --format json
llmff plugins list --plugin-dir examples/plugins
```

Protocol fixtures, a registry format, and trust guidance are published under
`docs/plugins/`:

- `docs/plugins/fixtures/protocol-v1/`: stdin/stdout examples plugin authors can
  copy into CI.
- `docs/plugins/registry.v1.json`: static registry entries for the official
  example plugins.
- `docs/plugins/registry.md`: registry schema notes.
- `docs/plugins/trust.md`: permissions, sandbox expectations, review checklist,
  and optional future plugin-signing guidance.

Run the fixture and registry checker:

```sh
scripts/check-plugin-fixtures.sh
LLMFF_BIN=llmff scripts/check-plugin-fixtures.sh --plugin-dir path/to/plugins
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

The current plugin protocol version is `1`.

Each capability is a separate process. llmff starts the entrypoint, writes one request to stdin, closes stdin, waits for exit, and reads stdout. A non-zero exit status fails the stage or backend call, and stderr is included in the user-facing error.

Stage plugins:

- Manifest kind: `stage`
- Pipeline op: `plugin:<capability-name>`
- stdin: parent stage text
- stdout: replacement stage text

Retrieval-provider, reranker, and postprocessor examples currently use this
generic stage protocol. llmff does not have separate native plugin kinds for
those stages in protocol `1`; the official examples name the capability for the
role they fill and document that mapping in the registry.

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

## Validation Output

`llmff plugins validate --plugin-dir <dir>` preserves the text behavior:

- prints `ok` on success
- exits non-zero with a human-readable error on failure

Use `--format json` for automation. JSON validation prints a report to stdout and exits non-zero when `valid` is `false`.

```json
{
  "format_version": 1,
  "plugin_protocol_version": 1,
  "plugin_dir": "examples/plugins",
  "valid": true,
  "plugin_count": 0,
  "plugins": [],
  "conformance_checks": [],
  "diagnostics": []
}
```

Diagnostics are structured records. `code` is stable for automation, `message` is for humans, and capability fields are present when the diagnostic applies to a specific capability.
`conformance_checks` is a static checklist for each capability. It records
whether the entrypoint is executable, which plugin protocol contract applies,
which JSON output contract the capability must satisfy, how failures should be
reported, and the explicit trust-boundary warning. These checks do not execute
plugin code.

```json
{
  "severity": "error",
  "code": "missing_entrypoint",
  "message": "plugin manifest `plugins/example/llmff-plugin.yaml` capability `stage` `text.uppercase` has missing entrypoint `plugins/example/./bin/uppercase`",
  "manifest_path": "plugins/example/llmff-plugin.yaml",
  "plugin_name": "example",
  "capability_kind": "stage",
  "capability_name": "text.uppercase",
  "entrypoint": "plugins/example/./bin/uppercase"
}
```

Known diagnostic codes:

- `manifest_read_failed`
- `manifest_parse_failed`
- `manifest_invalid`
- `missing_entrypoint`
- `entrypoint_not_executable`

Known conformance check codes:

- `command_protocol_v1`
- `entrypoint_executable`
- `schema_output_contract`
- `error_handling_contract`
- `trust_boundary_review`

## Protocol Compatibility

Plugin protocol `1` covers the manifest schema, capability kinds, entrypoint resolution, stdin/stdout process lifecycle, backend and sampler JSON request/response contracts, and validation report schema described above.

Compatibility policy:

- llmff keeps protocol `1` behavior backward compatible within the current major CLI line.
- Additive changes may add new fields to JSON objects. Plugin authors should ignore fields they do not understand.
- Breaking changes require a new plugin protocol version and documentation for migration.
- Signing is intentionally not part of protocol `1`. Plugin validation reports
  the trust boundary explicitly and checks structure, local entrypoints, and
  host executability without running plugin code.

## Security Boundaries

Plugins are local executables. llmff does not sandbox them. A plugin can read files available to the user, spawn processes, use the network, and write output wherever its OS permissions allow.

Install and run plugins only from trusted sources. Prefer checked-in scripts, pinned dependencies, and reviewable source. Avoid manifests that point at mutable global commands unless that is intentional. Use `llmff plugins validate` to catch malformed manifests, missing entrypoints, non-executable entrypoints, and static conformance warnings before a run; it is not a security scanner.

## Examples

See `examples/plugins` for minimal working examples:

- `stage-uppercase`: `stage` capability.
- `retrieval-static`: retrieval-provider example implemented as a `stage`
  capability.
- `reranker-length`: reranker example implemented as a `stage` capability.
- `backend-echo`: `backend` capability.
- `sampler-small`: `sampler` capability.
- `tool-stdio-cat`: `tool-transport` capability.
- `postprocessor-strip`: postprocessor example implemented as a `stage`
  capability.
