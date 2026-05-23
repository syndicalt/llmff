#!/usr/bin/env bash
set -euo pipefail

binary="${LLMFF_BIN:-llmff}"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
manifest="$repo_root/examples/providers/ollama.yaml"
output="$repo_root/examples/providers/ollama.answer.json"
base_url="${OLLAMA_BASE_URL:-http://localhost:11434}"

if [[ "${LLMFF_LIVE_PROVIDER_SMOKE:-}" != "1" ]]; then
  echo "skipping Ollama provider smoke; set LLMFF_LIVE_PROVIDER_SMOKE=1 to opt in"
  exit 0
fi

cleanup() {
  rm -f "$output"
}
trap cleanup EXIT

"$binary" run "$manifest" --ollama "ollama=$base_url"

test -s "$output"
echo "Ollama provider smoke passed"
