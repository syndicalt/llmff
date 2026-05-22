#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/check-windows-signing-wiring.sh

Verifies that Windows Authenticode signing helpers remain available, while
v0.1.2 release artifact publication is documented as unsigned.
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
  if ! grep -Fq -- "$needle" "$file"; then
    printf 'error: %s must contain: %s\n' "$file" "$needle" >&2
    exit 1
  fi
}

require_file 'scripts/sign-windows-msi.ps1'
require_text 'scripts/sign-windows-msi.ps1' 'Import-PfxCertificate'
require_text 'scripts/sign-windows-msi.ps1' 'signtool'
require_text 'scripts/sign-windows-msi.ps1' 'verify /pa /v'
require_file 'scripts/sign-windows-binary.ps1'
require_text 'scripts/sign-windows-binary.ps1' 'Import-PfxCertificate'
require_text 'scripts/sign-windows-binary.ps1' 'signtool'
require_text 'scripts/sign-windows-binary.ps1' 'verify /pa /v'
require_text '.github/workflows/release-artifacts.yml' 'scripts/smoke-windows-msi.sh --msi "dist/llmff-${version}-${{ matrix.target }}.msi"'
require_text 'docs/platform-support.md' 'unsigned `.zip` and unsigned `.msi`'
require_text 'docs/roadmap.md' 'Windows Authenticode signing remains a future paid distribution track'
