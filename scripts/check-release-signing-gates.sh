#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/check-release-signing-gates.sh --platform <windows|macos>

Fails when release signing or notarization credentials for the selected
platform are absent. This is a release gate, not a signing implementation.
USAGE
}

if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
  usage
  exit 0
fi

if [ "$#" -ne 2 ] || [ "$1" != "--platform" ]; then
  usage >&2
  exit 2
fi

platform="$2"

require_env() {
  local name="$1"
  if [ -z "${!name:-}" ]; then
    printf 'error: missing required %s signing secret: %s\n' "$platform" "$name" >&2
    exit 1
  fi
}

case "$platform" in
  windows)
    require_env WINDOWS_CODESIGN_CERT_P12_BASE64
    require_env WINDOWS_CODESIGN_CERT_PASSWORD
    require_env WINDOWS_CODESIGN_TIMESTAMP_URL
    ;;
  macos)
    require_env APPLE_DEVELOPER_ID_INSTALLER
    require_env APPLE_INSTALLER_CERT_P12_BASE64
    require_env APPLE_INSTALLER_CERT_PASSWORD
    require_env APPLE_ID
    require_env APPLE_TEAM_ID
    require_env APPLE_APP_SPECIFIC_PASSWORD
    ;;
  *)
    printf 'error: unsupported signing platform: %s\n' "$platform" >&2
    usage >&2
    exit 2
    ;;
esac

printf '%s signing gate prerequisites are present\n' "$platform"
