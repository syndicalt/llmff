#!/bin/sh
set -eu

plugin_dir="examples/plugins"

while [ "$#" -gt 0 ]; do
  case "$1" in
    --plugin-dir)
      if [ "$#" -lt 2 ]; then
        echo "missing value for --plugin-dir" >&2
        exit 2
      fi
      plugin_dir="$2"
      shift 2
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
registry="$root/docs/plugins/registry.v1.json"
fixtures="$root/docs/plugins/fixtures/protocol-v1"

case "$plugin_dir" in
  /*) resolved_plugin_dir="$plugin_dir" ;;
  *) resolved_plugin_dir="$root/$plugin_dir" ;;
esac

if [ "${LLMFF_BIN:-}" ]; then
  set -- "$LLMFF_BIN"
else
  set -- cargo run -q -p llmff --
fi

"$@" plugins validate --plugin-dir "$resolved_plugin_dir" >/dev/null

python3 - "$registry" "$fixtures" <<'PY'
import json
import pathlib
import sys

registry_path = pathlib.Path(sys.argv[1])
fixtures = pathlib.Path(sys.argv[2])

registry = json.loads(registry_path.read_text())
if registry.get("format_version") != 1:
    raise SystemExit("registry format_version must be 1")
if registry.get("plugin_protocol_version") != 1:
    raise SystemExit("registry plugin_protocol_version must be 1")

required = {
    "retrieval-provider",
    "reranker",
    "model-backend",
    "sampler",
    "tool-transport",
    "postprocessor",
}
categories = {plugin.get("category") for plugin in registry.get("plugins", [])}
missing = sorted(required - categories)
if missing:
    raise SystemExit(f"registry missing categories: {', '.join(missing)}")

for path in fixtures.rglob("*.json"):
    json.loads(path.read_text())

print("plugin fixtures ok")
PY
