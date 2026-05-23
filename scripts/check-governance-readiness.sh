#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$repo_root"

require_file() {
  local path="$1"
  if [ ! -f "$path" ]; then
    printf 'error: missing governance readiness artifact: %s\n' "$path" >&2
    exit 1
  fi
}

require_text() {
  local path="$1"
  local text="$2"
  require_file "$path"
  if ! grep -Fq -- "$text" "$path"; then
    printf 'error: %s must contain: %s\n' "$path" "$text" >&2
    exit 1
  fi
}

require_file "docs/governance.md"
require_file "CONTRIBUTING.md"
require_file "docs/distribution-trust.md"
require_file "docs/release-readiness.md"
require_file "docs/package-manager-roadmap.md"
require_file "docs/ecosystem-readiness.md"
require_file "docs/manifest-reproducibility.md"
require_file "docs/apt-repository-design.md"
require_file "docs/provider-smoke-readiness.md"
require_file "scripts/generate-release-trust-manifest.sh"
require_file "scripts/check-ecosystem-readiness.sh"
require_file "scripts/check-manifest-reproducibility.sh"
require_file "scripts/check-apt-repository-design.sh"
require_file "scripts/check-provider-smoke-readiness.sh"

require_text "docs/package-manager-roadmap.md" "publish only when maintainers decide the channel is support-ready"
require_text "docs/package-manager-roadmap.md" "apt stays parked until signing, repository metadata, hosting, key rotation, and recovery are designed"
require_text "docs/distribution-trust.md" "Authenticode and Apple notarization stay parked until paid credentials are available"
require_text "docs/distribution-trust.md" "SBOM and provenance readiness gate"
require_text "docs/distribution-trust.md" "llmff-<version>-release-trust.json"
require_text ".github/workflows/release-artifacts.yml" "scripts/generate-release-trust-manifest.sh"
require_text ".github/workflows/release-artifacts.yml" "release-trust.json"
require_text "docs/governance.md" "Manifest schema stability"
require_text "docs/governance.md" "Plugin protocol stability"
require_text "docs/governance.md" "CLI flag stability"
require_text "docs/governance.md" "Trace and event field stability"
require_text "docs/governance.md" "Deprecation policy"
require_text "CONTRIBUTING.md" "Stages"
require_text "CONTRIBUTING.md" "Plugins"
require_text "CONTRIBUTING.md" "Providers"
require_text "docs/release-readiness.md" "Ecosystem compatibility checklist"
require_text "docs/ecosystem-readiness.md" "Integration Gates"
require_text "docs/manifest-reproducibility.md" "manifest lockfile remains parked"
require_text "docs/apt-repository-design.md" "signed repository metadata"
require_text "docs/provider-smoke-readiness.md" "certification is a support commitment"

printf 'governance readiness validation succeeded\n'
