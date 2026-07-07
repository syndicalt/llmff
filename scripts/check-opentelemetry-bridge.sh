#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$SCRIPT_DIR/lib/checks.sh"
REQUIRE_FILE_LABEL="missing OpenTelemetry bridge artifact"

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$repo_root"

guide="docs/opentelemetry-bridge.md"

require_file "$guide"
require_file "docs/observability.md"
require_file "docs/events.md"
require_file "scripts/trace-to-summary.sh"
require_file "scripts/trace-to-metrics.sh"
require_file "examples/supervision/fixtures/success-trace.jsonl"
require_file "examples/supervision/fixtures/backend-error-trace.jsonl"

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
require_text "docs/observability.md" "output artifact locations"
require_text "docs/observability.md" "cache artifact locations"
require_text "docs/roadmap.md" "future OpenTelemetry integration"
require_text "docs/ecosystem-readiness.md" "OpenTelemetry bridge"
require_text "docs/ecosystem-readiness.md" "scripts/check-opentelemetry-bridge.sh"

success_summary="$(scripts/trace-to-summary.sh examples/supervision/fixtures/success-trace.jsonl)"
success_metrics="$(scripts/trace-to-metrics.sh examples/supervision/fixtures/success-trace.jsonl)"
failure_summary="$(scripts/trace-to-summary.sh examples/supervision/fixtures/backend-error-trace.jsonl)"
failure_metrics="$(scripts/trace-to-metrics.sh examples/supervision/fixtures/backend-error-trace.jsonl)"

case "$success_summary" in
  *"artifacts outputs="*"caches="*) ;;
  *) printf 'error: success summary must report output and cache artifact counts\n' >&2; exit 1 ;;
esac
case "$success_summary" in
  *"tokens prompt="*"completion="*"total="*) ;;
  *) printf 'error: success summary must report token totals\n' >&2; exit 1 ;;
esac
case "$success_metrics" in
  *"llmff_run_duration_ms "*"llmff_stage_duration_ms_sum "*"llmff_tokens_total "*) ;;
  *) printf 'error: success metrics missing run, stage, or token metrics\n' >&2; exit 1 ;;
esac
case "$failure_summary" in
  *"failure kind=backend"*) ;;
  *) printf 'error: failure summary must preserve backend failure kind\n' >&2; exit 1 ;;
esac
case "$failure_metrics" in
  *"llmff_backend_errors_total 1"*"llmff_failures_total 1"*) ;;
  *) printf 'error: failure metrics must count backend failures\n' >&2; exit 1 ;;
esac

printf 'OpenTelemetry bridge validation succeeded\n'
