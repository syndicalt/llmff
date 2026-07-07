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

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$SCRIPT_DIR/lib/checks.sh"

doc="docs/platform-support.md"
workspace_version="$(
  sed -n 's/^version = "\([^"]*\)"$/\1/p' Cargo.toml | head -n 1
)"
release_tag="v${workspace_version}"

require_file "$doc"

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
require_text "$doc" 'scripts/release-preflight.sh'
require_text "$doc" 'unsigned `.zip` and unsigned `.msi`'
require_text "$doc" 'unsigned `.pkg`'
require_text "$doc" 'Apple Developer ID signing and notarization remain a future paid distribution track'
require_text "$doc" 'Windows Authenticode signing remains a future paid distribution track'
require_text "$doc" 'llmff inspect examples/json-repair.yaml'

require_text 'README.md' 'docs/platform-support.md'
require_text 'README.md' "scripts/release-preflight.sh ${release_tag}"
require_text 'docs/release-readiness.md' 'docs/platform-support.md'
require_text 'docs/release-readiness.md' "scripts/release-preflight.sh ${release_tag}"

if grep -Eq 'Packaged installers .*on the roadmap|Future Capability Tracks|--mock llmff:good|Add published-asset verification' README.md docs/roadmap.md docs/platform-support.md; then
  printf 'error: release docs still describe completed package or capability tracks as future work\n' >&2
  exit 1
fi
