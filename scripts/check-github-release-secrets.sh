#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/check-github-release-secrets.sh [--repo <owner/repo>]

Verifies that the repository has the GitHub Actions secrets required for
tag-triggered Windows signing and macOS signing/notarization release jobs.
USAGE
}

repo="${GITHUB_REPOSITORY:-syndicalt/llmff}"

while [ "$#" -gt 0 ]; do
  case "$1" in
    --repo)
      if [ "$#" -lt 2 ] || [ -z "$2" ]; then
        usage >&2
        exit 2
      fi
      repo="$2"
      shift 2
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      usage >&2
      exit 2
      ;;
  esac
done

if ! command -v gh >/dev/null 2>&1; then
  printf 'error: gh is required to inspect repository release secrets\n' >&2
  exit 1
fi

required_secrets=(
  WINDOWS_CODESIGN_CERT_P12_BASE64
  WINDOWS_CODESIGN_CERT_PASSWORD
  WINDOWS_CODESIGN_TIMESTAMP_URL
  APPLE_DEVELOPER_ID_INSTALLER
  APPLE_INSTALLER_CERT_P12_BASE64
  APPLE_INSTALLER_CERT_PASSWORD
  APPLE_ID
  APPLE_TEAM_ID
  APPLE_APP_SPECIFIC_PASSWORD
)

secret_list="$(gh secret list --repo "$repo" | awk '{print $1}')"
missing=0

for secret in "${required_secrets[@]}"; do
  if ! grep -Fxq "$secret" <<<"$secret_list"; then
    printf 'error: repository %s is missing required release secret: %s\n' "$repo" "$secret" >&2
    missing=1
  fi
done

if [ "$missing" -ne 0 ]; then
  exit 1
fi

printf 'GitHub release secrets are configured for %s\n' "$repo"
