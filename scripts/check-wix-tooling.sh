#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/check-wix-tooling.sh

Verifies that Windows MSI packaging uses the repo-pinned WiX dotnet tool
instead of an unpinned global tool install.
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

  if ! grep -Fq -- "$needle" "$file"; then
    printf 'error: %s does not mention required text: %s\n' "$file" "$needle" >&2
    exit 1
  fi
}

reject_text() {
  local file="$1"
  local needle="$2"

  if grep -Fq -- "$needle" "$file"; then
    printf 'error: %s still contains forbidden text: %s\n' "$file" "$needle" >&2
    exit 1
  fi
}

require_file '.config/dotnet-tools.json'
require_text '.config/dotnet-tools.json' '"wix"'
require_text '.config/dotnet-tools.json' '"version": "5.0.2"'
require_text '.github/workflows/release-artifacts.yml' 'dotnet tool restore'
reject_text '.github/workflows/release-artifacts.yml' 'dotnet tool install --global wix'
require_text 'scripts/package-windows-msi.sh' 'dotnet tool restore'
require_text 'scripts/package-windows-msi.sh' 'dotnet wix build'
require_text 'README.md' 'dotnet tool restore'
