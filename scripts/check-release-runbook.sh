#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$SCRIPT_DIR/lib/checks.sh"
REQUIRE_FILE_LABEL="missing release runbook artifact"

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$repo_root"

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
  require_release_commit_matches_tag "docs/release-evidence/v0.8.0.md" "v0.8.0"
  require_pattern "docs/release-evidence/v0.8.0.md" 'Main CI workflow: GitHub Actions run `[0-9]+`' \
    "must record the main CI run id"
  require_pattern "docs/release-evidence/v0.8.0.md" 'Release artifacts workflow: GitHub Actions run `[0-9]+`' \
    "must record the release artifacts CI run id"
  require_text "docs/release-evidence/v0.8.0.md" "release asset verification succeeded for v0.8.0"
  require_text "docs/release-evidence/v0.8.0.md" "run cli-run succeeded"
fi

if grep -Fq -- "- [x] **Step 4: Ship \`v1.0.0\` only after compatibility review**" \
  docs/superpowers/plans/2026-05-30-llmff-v1-roadmap.md; then
  require_text "docs/superpowers/plans/2026-05-30-llmff-v1-roadmap.md" "scripts/check-release-assets.sh v1.0.0"
  require_text "docs/superpowers/plans/2026-05-30-llmff-v1-roadmap.md" "scripts/smoke-install.sh --git https://github.com/syndicalt/llmff --tag v1.0.0"
  require_text "docs/superpowers/plans/2026-05-30-llmff-v1-roadmap.md" "docs/release-evidence/v1.0.0.md"
  require_text "docs/release-evidence/v1.0.0.md" "scripts/release-preflight.sh v1.0.0"
  require_release_commit_matches_tag "docs/release-evidence/v1.0.0.md" "v1.0.0"
  require_pattern "docs/release-evidence/v1.0.0.md" 'Main CI workflow: GitHub Actions run `[0-9]+`' \
    "must record the main CI run id"
  require_pattern "docs/release-evidence/v1.0.0.md" 'Release artifacts workflow: GitHub Actions run `[0-9]+`' \
    "must record the release artifacts CI run id"
  require_text "docs/release-evidence/v1.0.0.md" "scripts/check-release-assets.sh v1.0.0"
  require_text "docs/release-evidence/v1.0.0.md" "release asset verification succeeded for v1.0.0"
  require_text "docs/release-evidence/v1.0.0.md" "scripts/smoke-install.sh --git https://github.com/syndicalt/llmff --tag v1.0.0"
  require_text "docs/release-evidence/v1.0.0.md" "run cli-run succeeded"
  require_text "docs/release-evidence/v1.0.0.md" "dependency and security review"
fi

printf 'release runbook validation succeeded\n'
