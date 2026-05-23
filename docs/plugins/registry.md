# Static Plugin Registry

`docs/plugins/registry.v1.json` is a static registry format for curated plugin
metadata. It is intentionally plain JSON so documentation sites, package indexes,
and CI jobs can consume it without running plugin code.

Required top-level fields:

- `format_version`: registry schema version. The current version is `1`.
- `plugin_protocol_version`: llmff plugin protocol version. The current version
  is `1`.
- `plugins`: array of registry entries.

Required plugin entry fields:

- `name`: plugin manifest name.
- `version`: plugin manifest version.
- `category`: ecosystem category such as `model-backend` or `tool-transport`.
- `manifest`: relative path or URL to `llmff-plugin.yaml`.
- `capabilities`: array of `{ "kind": "...", "name": "..." }` records.
- `summary`: short human-readable description.

The registry is advisory. llmff plugin loading still uses `--plugin-dir` and the
plugin manifests on disk.
