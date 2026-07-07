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

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$SCRIPT_DIR/lib/checks.sh"

require_file 'scripts/smoke-windows-msi.sh'
require_text '.github/workflows/release-artifacts.yml' 'scripts/smoke-windows-msi.sh --payload-root "$payload_root"'
require_text 'docs/platform-support.md' 'scripts/smoke-windows-msi.sh'
require_text 'docs/roadmap.md' 'Windows MSI smoke tests'
require_text 'README.md' 'scripts/smoke-windows-msi.sh --payload-root'
