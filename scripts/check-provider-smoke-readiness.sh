#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$repo_root"

require_file() {
  local path="$1"
  if [ ! -f "$path" ]; then
    printf 'error: missing provider smoke readiness artifact: %s\n' "$path" >&2
    exit 1
  fi
}

require_text() {
  local path="$1"
  local text="$2"
  require_file "$path"
  if ! grep -Fq -- "$text" "$path"; then
    printf 'error: %s must contain: %s\n' "$path" "$text" >&2
    exit 1
  fi
}

require_absent_text() {
  local path="$1"
  local text="$2"
  require_file "$path"
  if grep -Fq -- "$text" "$path"; then
    printf 'error: %s must not contain: %s\n' "$path" "$text" >&2
    exit 1
  fi
}

guide="docs/provider-smoke-readiness.md"
workflow=".github/workflows/live-provider-smoke.yml"
openai_script="scripts/smoke-openai-compatible-provider.sh"
ollama_script="scripts/smoke-ollama-provider.sh"

require_file "$guide"
require_file "$workflow"
require_file "$openai_script"
require_file "$ollama_script"

for text in \
  "LLMFF_LIVE_PROVIDER_SMOKE=1" \
  "OPENAI_API_KEY" \
  "OPENAI_BASE_URL" \
  "OLLAMA_BASE_URL" \
  "ubuntu-latest" \
  "workflow_dispatch" \
  "not run on pull_request or push" \
  "certification is a support commitment"
do
  require_text "$guide" "$text"
done

require_text "$workflow" "workflow_dispatch"
require_absent_text "$workflow" "pull_request"
require_absent_text "$workflow" "push:"
require_text "$workflow" "runs-on: ubuntu-latest"
require_text "$workflow" "LLMFF_LIVE_PROVIDER_SMOKE: \"1\""
require_text "$workflow" "secrets.OPENAI_API_KEY"
require_text "$workflow" "vars.OPENAI_BASE_URL"
require_text "$workflow" "OLLAMA_BASE_URL: http://localhost:11434"

require_text "$openai_script" "LLMFF_LIVE_PROVIDER_SMOKE"
require_text "$openai_script" "OPENAI_API_KEY"
require_text "$openai_script" "OPENAI_BASE_URL"
require_text "$openai_script" "exit 0"
require_text "$ollama_script" "LLMFF_LIVE_PROVIDER_SMOKE"
require_text "$ollama_script" "OLLAMA_BASE_URL"
require_text "$ollama_script" "exit 0"

printf 'provider smoke readiness validation succeeded\n'
