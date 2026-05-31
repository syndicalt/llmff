#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$repo_root"

require_text() {
  local file="$1"
  local text="$2"
  if [ ! -f "$file" ]; then
    printf 'error: missing release runbook artifact: %s\n' "$file" >&2
    exit 1
  fi
  if ! grep -Fq -- "$text" "$file"; then
    printf 'error: %s must contain: %s\n' "$file" "$text" >&2
    exit 1
  fi
}

require_text "docs/release-runbook.md" "Local preparation"
require_text "docs/release-runbook.md" "Published release candidate"
require_text "docs/release-runbook.md" "Final v1.0 release"
require_text "docs/release-runbook.md" "scripts/release-preflight.sh v0.8.0"
require_text "docs/release-runbook.md" "git tag -a v0.8.0"
require_text "docs/release-runbook.md" "git push origin v0.8.0"
require_text "docs/release-runbook.md" "scripts/check-release-assets.sh v0.8.0"
require_text "docs/release-runbook.md" "scripts/smoke-install.sh --git https://github.com/syndicalt/llmff --tag v0.8.0"
require_text "docs/release-runbook.md" "commit SHA, tag, CI run URL"
require_text "docs/release-runbook.md" "Do not publish \`v1.0.0\` until"
require_text "docs/release-runbook.md" "dependency and security review"
require_text "docs/release-evidence/v1.0.0-security.md" "cargo audit --format json"
require_text "docs/release-evidence/v1.0.0-security.md" "found=false"
require_text "docs/release-evidence/v1.0.0-security.md" "count=0"
require_text "docs/release-evidence/v1.0.0-security.md" "cargo tree -d"
require_text "docs/release-runbook.md" "scripts/check-release-assets.sh v1.0.0"
require_text "docs/release-runbook.md" "scripts/smoke-install.sh --git https://github.com/syndicalt/llmff --tag v1.0.0"
require_text "docs/release-readiness.md" "docs/release-runbook.md"
require_text "docs/superpowers/plans/2026-05-30-llmff-v1-roadmap.md" "docs/release-runbook.md"

if grep -Fq -- "- [x] **Step 3: Cut at least one release candidate**" \
  docs/superpowers/plans/2026-05-30-llmff-v1-roadmap.md; then
  require_text "docs/superpowers/plans/2026-05-30-llmff-v1-roadmap.md" "scripts/check-release-assets.sh v0.8.0"
  require_text "docs/superpowers/plans/2026-05-30-llmff-v1-roadmap.md" "scripts/smoke-install.sh --git https://github.com/syndicalt/llmff --tag v0.8.0"
  require_text "docs/superpowers/plans/2026-05-30-llmff-v1-roadmap.md" "docs/release-evidence/v0.8.0.md"
  require_text "docs/release-evidence/v0.8.0.md" "95cd3ebf16cb6e0e6630fba29da183d47e55424f"
  require_text "docs/release-evidence/v0.8.0.md" "26723385951"
  require_text "docs/release-evidence/v0.8.0.md" "release asset verification succeeded for v0.8.0"
  require_text "docs/release-evidence/v0.8.0.md" "run cli-run succeeded"
fi

if grep -Fq -- "- [x] **Step 4: Ship \`v1.0.0\` only after compatibility review**" \
  docs/superpowers/plans/2026-05-30-llmff-v1-roadmap.md; then
  require_text "docs/superpowers/plans/2026-05-30-llmff-v1-roadmap.md" "scripts/check-release-assets.sh v1.0.0"
  require_text "docs/superpowers/plans/2026-05-30-llmff-v1-roadmap.md" "scripts/smoke-install.sh --git https://github.com/syndicalt/llmff --tag v1.0.0"
  require_text "docs/superpowers/plans/2026-05-30-llmff-v1-roadmap.md" "docs/release-evidence/v1.0.0.md"
  require_text "docs/release-evidence/v1.0.0.md" "scripts/release-preflight.sh v1.0.0"
  require_text "docs/release-evidence/v1.0.0.md" "18eb62a18d40935eb1fc0b07109ff0eba3807edb"
  require_text "docs/release-evidence/v1.0.0.md" "26723793883"
  require_text "docs/release-evidence/v1.0.0.md" "26723827131"
  require_text "docs/release-evidence/v1.0.0.md" "scripts/check-release-assets.sh v1.0.0"
  require_text "docs/release-evidence/v1.0.0.md" "release asset verification succeeded for v1.0.0"
  require_text "docs/release-evidence/v1.0.0.md" "scripts/smoke-install.sh --git https://github.com/syndicalt/llmff --tag v1.0.0"
  require_text "docs/release-evidence/v1.0.0.md" "run cli-run succeeded"
  require_text "docs/release-evidence/v1.0.0.md" "dependency and security review"
fi

printf 'release runbook validation succeeded\n'
