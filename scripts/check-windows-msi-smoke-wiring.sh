#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/check-windows-msi-smoke-wiring.sh

Verifies that Windows MSI smoke testing is wired into release artifacts and
user-facing release documentation.
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

require_file 'scripts/smoke-windows-msi.sh'
require_text '.github/workflows/release-artifacts.yml' 'scripts/smoke-windows-msi.sh --msi'
require_text 'docs/platform-support.md' 'scripts/smoke-windows-msi.sh'
require_text 'docs/roadmap.md' 'Windows MSI smoke tests'
require_text 'README.md' 'scripts/smoke-windows-msi.sh --payload-root'
