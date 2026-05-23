#!/usr/bin/env bash
set -euo pipefail

binary="${LLMFF_BIN:-llmff}"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
manifest="$repo_root/examples/providers/openai-compatible.yaml"
output="$repo_root/examples/providers/openai-compatible.answer.json"
base_url="${OPENAI_BASE_URL:-https://api.openai.com/v1}"
api_key_env="${OPENAI_API_KEY_ENV:-OPENAI_API_KEY}"

if [[ "${LLMFF_LIVE_PROVIDER_SMOKE:-}" != "1" ]]; then
  echo "skipping OpenAI-compatible provider smoke; set LLMFF_LIVE_PROVIDER_SMOKE=1 to opt in"
  exit 0
fi

if [[ -z "${!api_key_env:-}" ]]; then
  echo "skipping OpenAI-compatible provider smoke; ${api_key_env} is not set"
  exit 0
fi

cleanup() {
  rm -f "$output"
}
trap cleanup EXIT

"$binary" run "$manifest" \
  --backend "openai=$base_url" \
  --api-key-env "openai=$api_key_env"

test -s "$output"
echo "OpenAI-compatible provider smoke passed"
