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
workflows="docs/agent-workflows.md"
harness_contract="docs/agent-harness-contract.md"
harness_examples="examples/agent-harnesses/README.md"

require_file "$guide"
require_file "$workflows"
require_file "$harness_contract"
require_file "$harness_examples"
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

for text in \
  "## Canonical Subprocess Patterns" \
  "Short Jobs" \
  "Long Jobs" \
  "Batch Jobs" \
  "Streaming Jobs" \
  "batch mode can use \`--run-dir\`" \
  "preserve the original llmff exit code" \
  "run_failed.failure_kind"
do
  require_text "$workflows" "$text"
done

for text in \
  "batch mode can use \`--run-dir\`" \
  "preserve the original exit code" \
  "unknown values" \
  "run_failed.failure_kind"
do
  require_text "$harness_contract" "$text"
done

for text in \
  "## Preserve Exit Codes" \
  "host timeout" \
  "batch mode can use \`--run-dir\`" \
  "run_failed.failure_kind"
do
  require_text "$guide" "$text"
done

for text in \
  "Short jobs" \
  "Long jobs" \
  "Batch jobs" \
  "Streaming jobs" \
  "Do not translate non-zero llmff statuses into framework-specific success"
do
  require_text "$harness_examples" "$text"
done

require_text "docs/ecosystem-readiness.md" "Agent runner adoption"
require_text "docs/ecosystem-readiness.md" "scripts/check-agent-adoption-guide.sh"
require_text "docs/roadmap.md" "Add adoption-oriented guides only when they demonstrate real integration"

printf 'agent adoption guide validation succeeded\n'
