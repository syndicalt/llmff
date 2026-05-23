#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$repo_root"

require_file() {
  local path="$1"
  if [ ! -f "$path" ]; then
    printf 'error: missing manifest reproducibility artifact: %s\n' "$path" >&2
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

guide="docs/manifest-reproducibility.md"
schema="docs/schemas/inspect-report-v1.schema.json"
fixture="fixtures/golden/inspect/report.json"

require_file "$guide"
require_file "$schema"
require_file "$fixture"

for text in \
  "manifest hash" \
  "resolved inputs" \
  "resolved outputs" \
  "stage order" \
  "backend aliases" \
  "model ids" \
  "plugin dependencies" \
  "cache policy" \
  "checkpoint/resume policy" \
  "manifest lockfile remains parked" \
  "materially improves portability"
do
  require_text "$guide" "$text"
done

for text in \
  '"hash"' \
  '"inputs"' \
  '"outputs"' \
  '"stage_order"' \
  '"backends"' \
  '"model"' \
  '"plugins"' \
  '"cache_policy"' \
  '"checkpoint"' \
  '"execution"'
do
  require_text "$schema" "$text"
  require_text "$fixture" "$text"
done

require_text "docs/roadmap.md" "Explore lockfile or manifest-lock support only if it materially improves"
require_text "docs/roadmap.md" "Maintain schema compatibility fixtures for every additive manifest contract"

printf 'manifest reproducibility validation succeeded\n'
