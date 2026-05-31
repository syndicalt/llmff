# Plugin Template

This directory is a copyable protocol version 1 plugin template. It declares
one minimal example for each supported capability kind:

- `stage`: `template.uppercase`
- `backend`: `template-echo`
- `sampler`: `template-deterministic`
- `tool-transport`: `template-stdio`

Validate it with the rest of the examples:

```bash
llmff plugins validate --plugin-dir examples/plugins
scripts/check-plugin-fixtures.sh
```

The entrypoints are intentionally small shell scripts. They demonstrate process
shape, stdin/stdout ownership, and JSON output contracts; they are not a
security boundary. Plugins are unsandboxed local executables and run with the
same operating-system permissions as `llmff`.

Protocol fixtures live in `docs/plugins/fixtures/protocol-v1`. Use those files
as CI examples for backend, sampler, stage, and tool-transport contracts.
