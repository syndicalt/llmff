#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$repo_root"

require_file() {
  local path="$1"
  if [ ! -f "$path" ]; then
    printf 'error: missing agent adoption artifact: %s\n' "$path" >&2
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

guide="docs/adoption/agent-runner.md"

require_file "$guide"
require_file "docs/agent-workflows.md"
require_file "examples/agent-workflows/supervisor.py"
require_file "examples/agent-workflows/batch-supervisor.py"
require_file "examples/agent-workflows/node-supervisor.mjs"

for text in \
  "bounded execution tool" \
  "preflight" \
  "dispatch" \
  "supervision" \
  "artifact collection" \
  "retry decision" \
  "failure_kind" \
  "exit code" \
  "checkpoint" \
  "trace" \
  "events" \
  "batch-supervisor.py" \
  "node-supervisor.mjs" \
  "do not read prompt payloads from metadata"
do
  require_text "$guide" "$text"
done

require_text "docs/ecosystem-readiness.md" "Agent runner adoption"
require_text "docs/ecosystem-readiness.md" "scripts/check-agent-adoption-guide.sh"
require_text "docs/roadmap.md" "Add adoption-oriented guides only when they demonstrate real integration"

printf 'agent adoption guide validation succeeded\n'
