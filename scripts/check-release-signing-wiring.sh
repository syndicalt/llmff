#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/check-release-signing-wiring.sh

Verifies that release signing and notarization gates are wired into release
metadata, docs, and tag-only release artifact CI.
USAGE
}

if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
  usage
  exit 0
fi

if [ "$#" -ne 0 ]; then
  usage >&2
  exit 2
fi

require_file() {
  local path="$1"
  if [ ! -f "$path" ]; then
    printf 'error: missing required file: %s\n' "$path" >&2
    exit 1
  fi
}

require_text() {
  local file="$1"
  local needle="$2"
  require_file "$file"
  if ! grep -Fq "$needle" "$file"; then
    printf 'error: %s must contain: %s\n' "$file" "$needle" >&2
    exit 1
  fi
}

require_file 'scripts/check-release-signing-gates.sh'
require_text '.github/workflows/release-artifacts.yml' 'Validate Windows signing gate'
require_text '.github/workflows/release-artifacts.yml' 'Validate macOS signing and notarization gate'
require_text '.github/workflows/release-artifacts.yml' 'scripts/check-release-signing-gates.sh --platform windows'
require_text '.github/workflows/release-artifacts.yml' 'scripts/check-release-signing-gates.sh --platform macos'
require_text '.github/workflows/release-artifacts.yml' 'WINDOWS_CODESIGN_CERT_P12_BASE64'
require_text '.github/workflows/release-artifacts.yml' 'APPLE_DEVELOPER_ID_INSTALLER'
require_text 'docs/platform-support.md' 'scripts/check-release-signing-gates.sh --platform windows'
require_text 'docs/platform-support.md' 'scripts/check-release-signing-gates.sh --platform macos'
require_text 'docs/release-readiness.md' 'signing and notarization release gates'
require_text 'docs/roadmap.md' 'Signing gate implementation slice'
