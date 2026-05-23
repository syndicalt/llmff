#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$repo_root"

require_file() {
  local path="$1"
  if [ ! -f "$path" ]; then
    printf 'error: missing ecosystem readiness artifact: %s\n' "$path" >&2
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

guide="docs/ecosystem-readiness.md"

require_file "$guide"
require_file "scripts/check-schema-contract.py"
require_file "scripts/check-manifest-reproducibility.sh"
require_file "scripts/check-plugin-fixtures.sh"
require_file "scripts/check-package-manager-metadata.sh"
require_file "scripts/check-release-publication-wiring.sh"
require_file "scripts/check-release-assets.sh"
require_file "scripts/check-provider-smoke-readiness.sh"
require_file ".github/workflows/live-provider-smoke.yml"
require_file "docs/schemas/pipeline-manifest-v1.schema.json"
require_file "docs/manifest-reproducibility.md"
require_file "docs/schemas/inspect-report-v1.schema.json"
require_file "docs/events.md"
require_file "docs/opentelemetry-bridge.md"
require_file "docs/plugins/registry.v1.json"
require_file "docs/provider-troubleshooting.md"
require_file "docs/provider-smoke-readiness.md"
require_file "docs/agent-workflows.md"
require_file "docs/adoption/agent-runner.md"
require_file "docs/package-manager-roadmap.md"
require_file "docs/distribution-trust.md"

for integration in \
  "Manifest contracts" \
  "Trace and event streams" \
  "OpenTelemetry bridge" \
  "CLI JSON output" \
  "Plugin protocol" \
  "Provider onboarding" \
  "Agent subprocess embedding" \
  "Agent runner adoption" \
  "Package-manager metadata" \
  "Release assets"
do
  require_text "$guide" "$integration"
done

for gate in \
  "python3 scripts/check-schema-contract.py" \
  "scripts/check-manifest-reproducibility.sh" \
  "cargo test -p llmff --test cli_run observability_export_scripts_summarize_trace_fixture" \
  "scripts/check-opentelemetry-bridge.sh" \
  "cargo test -p llmff --test cli_run inspect_json_reports_reproducible_execution_contract" \
  "scripts/check-plugin-fixtures.sh" \
  "scripts/check-provider-smoke-readiness.sh" \
  "cargo test -p llmff --test example_catalog agent_workflow_docs_link_to_a_runnable_supervisor_example" \
  "scripts/check-agent-adoption-guide.sh" \
  "scripts/check-package-manager-metadata.sh" \
  "scripts/check-release-publication-wiring.sh" \
  "scripts/check-release-assets.sh <tag>"
do
  require_text "$guide" "$gate"
done

require_text "$guide" "support commitments"
require_text "$guide" "explicitly opt-in"
require_text "docs/manifest-reproducibility.md" "manifest lockfile remains parked"
require_text "docs/provider-smoke-readiness.md" "certification is a support commitment"
require_text "docs/opentelemetry-bridge.md" "no network telemetry by default"
require_text "docs/adoption/agent-runner.md" "bounded execution tool"
require_text "docs/roadmap.md" "Keep every public integration path covered by a local validation gate or a"

printf 'ecosystem readiness validation succeeded\n'
