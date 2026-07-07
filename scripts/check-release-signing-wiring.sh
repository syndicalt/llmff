#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/check-release-signing-wiring.sh

Verifies that paid release signing and notarization are documented as deferred
and do not block tag-triggered unsigned release artifact publication.
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

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$SCRIPT_DIR/lib/checks.sh"

workspace_version="$(
  sed -n 's/^version = "\([^"]*\)"$/\1/p' Cargo.toml | head -n 1
)"
release_tag="v${workspace_version}"

require_file 'scripts/check-release-signing-gates.sh'
require_file 'scripts/check-github-release-secrets.sh'
require_text 'scripts/check-github-release-secrets.sh' 'WINDOWS_CODESIGN_CERT_P12_BASE64'
require_text 'scripts/check-github-release-secrets.sh' 'APPLE_DEVELOPER_ID_INSTALLER'
require_text 'scripts/check-github-release-secrets.sh' 'APPLE_APP_SPECIFIC_PASSWORD'
require_text 'scripts/release-preflight.sh' '--check-github-secrets'
require_text 'docs/platform-support.md' 'unsigned `.zip` and unsigned `.msi`'
require_text 'docs/platform-support.md' 'unsigned `.pkg`'
require_text 'docs/release-readiness.md' "Unsigned Windows and macOS artifacts are acceptable for ${release_tag}"
require_text 'docs/roadmap.md' 'Trusted signing and notarization remain a future paid distribution track'

for forbidden in \
  'Validate Windows signing gate' \
  'Validate macOS signing and notarization gate' \
  'Sign Windows release binary' \
  'Sign Windows MSI installer' \
  'Smoke signed Windows MSI installer' \
  'Sign and notarize macOS installer' \
  'Smoke signed and notarized macOS installer'; do
  if grep -Fq -- "$forbidden" '.github/workflows/release-artifacts.yml'; then
    printf 'error: release workflow still blocks unsigned publication with: %s\n' "$forbidden" >&2
    exit 1
  fi
done
