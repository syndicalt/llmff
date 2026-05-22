#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/release-preflight.sh <tag>

Checks local release metadata before creating or pushing a release tag.
The tag must match the workspace package version, have release notes, and keep
the documented install, packaging, and publication gates wired.

Example:
  scripts/release-preflight.sh v0.1.1
USAGE
}

if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
  usage
  exit 0
fi

if [ "$#" -ne 1 ]; then
  usage >&2
  exit 2
fi

tag="$1"

case "$tag" in
  v[0-9]*.[0-9]*.[0-9]*) ;;
  *)
    printf 'error: tag must be a semver tag like v0.1.1: %s\n' "$tag" >&2
    exit 2
    ;;
esac

version="${tag#v}"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$repo_root"

require_file() {
  local file="$1"
  if [ ! -f "$file" ]; then
    printf 'error: missing required release file: %s\n' "$file" >&2
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

workspace_version="$(
  sed -n 's/^version = "\([^"]*\)"$/\1/p' Cargo.toml | head -n 1
)"

if [ -z "$workspace_version" ]; then
  printf 'error: could not read workspace package version from Cargo.toml\n' >&2
  exit 1
fi

if [ "$workspace_version" != "$version" ]; then
  printf 'error: tag %s does not match workspace version %s\n' "$tag" "$workspace_version" >&2
  exit 1
fi

notes="docs/release-notes/${tag}.md"
require_file "$notes"
require_text "$notes" "# llmff ${tag}"
require_text "$notes" "cargo install --git https://github.com/syndicalt/llmff --tag ${tag} llmff"
require_text "$notes" "scripts/smoke-install.sh --git https://github.com/syndicalt/llmff --tag ${tag}"

require_text "README.md" "cargo install --git https://github.com/syndicalt/llmff --tag ${tag} llmff"
require_text "README.md" "scripts/release-preflight.sh ${tag}"
require_text "docs/release-readiness.md" "scripts/release-preflight.sh ${tag}"
require_text "docs/platform-support.md" "scripts/release-preflight.sh"

require_text ".github/workflows/release-artifacts.yml" 'gh release view "$GITHUB_REF_NAME"'
require_text ".github/workflows/release-artifacts.yml" 'gh release create "$GITHUB_REF_NAME"'
require_text ".github/workflows/release-artifacts.yml" 'gh release upload "$GITHUB_REF_NAME"'

bash scripts/check-wix-tooling.sh
bash scripts/check-platform-support-doc.sh
bash scripts/check-windows-msi-smoke-wiring.sh
bash scripts/check-release-publication-wiring.sh
bash scripts/check-release-signing-wiring.sh
bash scripts/check-windows-signing-wiring.sh

printf 'release preflight succeeded for %s\n' "$tag"
