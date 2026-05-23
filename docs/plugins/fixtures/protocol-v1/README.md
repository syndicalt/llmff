# Plugin Protocol v1 Fixtures

These fixtures are stable examples for plugin authors to keep in their own CI.
They document the stdin/stdout contracts for protocol version `1`.

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
plugin categories.
