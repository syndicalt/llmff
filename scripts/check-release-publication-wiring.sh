#!/usr/bin/env bash
set -euo pipefail

workflow=".github/workflows/release-artifacts.yml"
readiness="docs/release-readiness.md"

require_text() {
  local file="$1"
  local text="$2"
  if ! grep -Fq "$text" "$file"; then
    printf 'error: %s must contain: %s\n' "$file" "$text" >&2
    exit 1
  fi
}

require_text "$workflow" 'gh release view "$GITHUB_REF_NAME"'
require_text "$workflow" 'gh release create "$GITHUB_REF_NAME"'
require_text "$workflow" 'gh release upload "$GITHUB_REF_NAME"'
require_text "scripts/check-release-assets.sh" 'gh release view "$tag"'
require_text "scripts/check-release-assets.sh" 'gh release download "$tag"'
require_text "scripts/check-release-assets.sh" 'scripts/smoke-archive.sh'
require_text "$readiness" "creates the GitHub Release when"
require_text "$readiness" "the tag does not already have one"
require_text "$readiness" "scripts/check-release-assets.sh v0.1.2"
