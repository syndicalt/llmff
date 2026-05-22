#!/usr/bin/env bash
set -euo pipefail

workflow=".github/workflows/release-artifacts.yml"
readiness="docs/release-readiness.md"

require_text() {
  local file="$1"
  local text="$2"
  if ! grep -Fq -- "$text" "$file"; then
    printf 'error: %s must contain: %s\n' "$file" "$text" >&2
    exit 1
  fi
}

require_text "$workflow" 'gh release view "$GITHUB_REF_NAME"'
require_text "$workflow" 'gh release create "$GITHUB_REF_NAME"'
require_text "$workflow" 'gh release upload "$GITHUB_REF_NAME"'
require_text "$workflow" 'RELEASE_REPOSITORY: syndicalt/llmff'
require_text "$workflow" 'release publication must run from %s, got %s'
require_text "$workflow" '--repo "$RELEASE_REPOSITORY"'
require_text "$workflow" 'rm -rf release-assets/arch'
require_text "$workflow" 'find release-assets -maxdepth 1 -type f'
require_text "$workflow" 'release assets already uploaded for %s'
require_text "$workflow" 'publish-release:'
require_text "$workflow" 'needs: archive'
require_text "$workflow" 'actions/download-artifact@v4'
require_text "$workflow" 'llmff-${version}-arch.SRCINFO'
if awk '/^  publish-release:/{seen=0} /name: Upload archive artifacts/{seen=1} seen && /Ensure GitHub Release exists|Upload GitHub Release assets/{found=1} END{exit found ? 0 : 1}' "$workflow"; then
  printf 'error: archive matrix job must not create or upload GitHub Release assets directly\n' >&2
  exit 1
fi
require_text "scripts/check-release-assets.sh" 'gh release view "$tag"'
require_text "scripts/check-release-assets.sh" 'gh release download "$tag"'
require_text "scripts/check-release-assets.sh" 'scripts/smoke-archive.sh'
require_text "scripts/check-release-assets.sh" 'llmff-${version}-arch.SRCINFO'
require_text "$readiness" "creates the GitHub Release when"
require_text "$readiness" "the tag does not already have one"
require_text "$readiness" "scripts/check-release-assets.sh v0.1.2"
