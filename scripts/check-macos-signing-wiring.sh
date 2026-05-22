#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/check-macos-signing-wiring.sh

Verifies that macOS package signing, notarization, and stapling are wired into
tag-triggered release artifacts and local release preflight checks.
USAGE
}

if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
  usage
  exit 0
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$repo_root"

require_file() {
  local file="$1"
  if [ ! -f "$file" ]; then
    printf 'error: missing required file: %s\n' "$file" >&2
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

require_file 'scripts/sign-notarize-macos-pkg.sh'
require_text 'scripts/sign-notarize-macos-pkg.sh' 'security import'
require_text 'scripts/sign-notarize-macos-pkg.sh' 'productsign'
require_text 'scripts/sign-notarize-macos-pkg.sh' 'xcrun notarytool submit'
require_text 'scripts/sign-notarize-macos-pkg.sh' 'xcrun stapler staple'
require_text 'scripts/sign-notarize-macos-pkg.sh' 'pkgutil --check-signature'

require_text 'scripts/check-release-signing-gates.sh' 'APPLE_INSTALLER_CERT_P12_BASE64'
require_text 'scripts/check-release-signing-gates.sh' 'APPLE_INSTALLER_CERT_PASSWORD'

require_text '.github/workflows/release-artifacts.yml' 'Sign and notarize macOS installer'
require_text '.github/workflows/release-artifacts.yml' 'Smoke signed and notarized macOS installer'
require_text '.github/workflows/release-artifacts.yml' 'scripts/sign-notarize-macos-pkg.sh'
require_text '.github/workflows/release-artifacts.yml' 'APPLE_INSTALLER_CERT_P12_BASE64'
require_text '.github/workflows/release-artifacts.yml' 'APPLE_INSTALLER_CERT_PASSWORD'

require_text 'scripts/release-preflight.sh' 'bash scripts/check-macos-signing-wiring.sh'
require_text 'docs/platform-support.md' 'signed and notarized `.pkg`'
require_text 'docs/roadmap.md' 'macOS package signing and notarization implementation slice'
require_text 'docs/release-readiness.md' 'macOS release tags sign, notarize, staple, and smoke-test'

