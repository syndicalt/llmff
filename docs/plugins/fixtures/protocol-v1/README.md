# Plugin Protocol v1 Fixtures

These fixtures are stable examples for plugin authors to keep in their own CI.
They document the stdin/stdout contracts for protocol version `1`.

Fixtures:

- `backend/infer-request.json`: JSON request sent to backend plugins.
- `backend/infer-response.json`: JSON response expected from backend plugins.
- `sampler/infer-request.json`: JSON request sent to sampler plugins.
- `sampler/overrides.json`: JSON sampling overrides returned by sampler
  plugins.
- `stage/stdin.txt`: text stdin sent to generic stage plugins.
- `stage/stdout.txt`: text stdout returned by generic stage plugins.
- `tool-transport/stdin.txt`: text stdin sent to tool transport plugins.
- `tool-transport/stdout.txt`: text stdout returned by tool transport plugins.

Run the repository checker against the official examples:

```sh
scripts/check-plugin-fixtures.sh
```

Run it against your own plugin directory:

```sh
LLMFF_BIN=llmff scripts/check-plugin-fixtures.sh --plugin-dir path/to/plugins
```

The checker validates plugin manifests with `llmff plugins validate`, parses the
JSON fixtures, and verifies the static registry has entries for the official
plugin categories. It also enforces registry promotion policy, per-plugin review
evidence, and explicit trust-boundary metadata for promoted entries.
