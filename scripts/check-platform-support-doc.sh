#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/check-platform-support-doc.sh

Verifies that the platform support documentation covers the release artifact
targets and their installation assumptions.
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

doc="docs/platform-support.md"

if [ ! -f "$doc" ]; then
  printf 'error: missing %s\n' "$doc" >&2
  exit 1
fi

require_text() {
  local file="$1"
  local needle="$2"

  if ! grep -Fq "$needle" "$file"; then
    printf 'error: %s does not mention required text: %s\n' "$file" "$needle" >&2
    exit 1
  fi
}

require_text "$doc" 'x86_64-unknown-linux-gnu'
require_text "$doc" 'x86_64-pc-windows-msvc'
require_text "$doc" 'aarch64-apple-darwin'
require_text "$doc" 'x86_64-apple-darwin'
require_text "$doc" 'glibc'
require_text "$doc" 'Ubuntu and Debian'
require_text "$doc" 'Arch Linux'
require_text "$doc" 'unsigned'
require_text "$doc" 'cargo install --git https://github.com/syndicalt/llmff --tag'
require_text "$doc" 'scripts/smoke-archive.sh'
require_text "$doc" 'scripts/smoke-deb.sh'
require_text "$doc" 'scripts/smoke-macos-pkg.sh'

require_text 'README.md' 'docs/platform-support.md'
require_text 'docs/release-readiness.md' 'docs/platform-support.md'
