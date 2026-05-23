#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$repo_root"

require_file() {
  local path="$1"
  if [ ! -f "$path" ]; then
    printf 'error: missing OpenTelemetry bridge artifact: %s\n' "$path" >&2
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

guide="docs/opentelemetry-bridge.md"

require_file "$guide"
require_file "docs/observability.md"
require_file "docs/events.md"
require_file "scripts/trace-to-summary.sh"
require_file "scripts/trace-to-metrics.sh"

for text in \
  "future OpenTelemetry bridge" \
  "trace-to-metrics.sh" \
  "trace-to-summary.sh" \
  "file-based supervision contract" \
  "no collectors by default" \
  "no network telemetry by default" \
  "deployment-owned bridge" \
  "attribute mapping" \
  "payload exclusion" \
  "support commitment"
do
  require_text "$guide" "$text"
done

for text in \
  "llmff.run.id" \
  "llmff.manifest.hash" \
  "llmff.stage.id" \
  "llmff.stage.op" \
  "llmff.failure.kind"
do
  require_text "$guide" "$text"
done

require_text "docs/observability.md" "docs/opentelemetry-bridge.md"
require_text "docs/roadmap.md" "future OpenTelemetry integration"
require_text "docs/ecosystem-readiness.md" "OpenTelemetry bridge"
require_text "docs/ecosystem-readiness.md" "scripts/check-opentelemetry-bridge.sh"

printf 'OpenTelemetry bridge validation succeeded\n'
