#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$SCRIPT_DIR/lib/checks.sh"
REQUIRE_FILE_LABEL="missing apt repository design artifact"

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$repo_root"

# require_absent_path is only duplicated in 2 scripts (below the 3+ lib
# threshold); kept local per script rather than promoted to lib/checks.sh.
require_absent_path() {
  local path="$1"
  if [ -e "$path" ]; then
    printf 'error: apt repository metadata is parked and must not be shipped: %s\n' "$path" >&2
    exit 1
  fi
}

design="docs/apt-repository-design.md"

require_file "$design"
for text in \
  "signed repository metadata" \
  "InRelease" \
  "Release.gpg" \
  "key rotation" \
  "historical retention" \
  "hosting" \
  "recovery" \
  "no apt repository installation instructions" \
  "post-publication verifier" \
  "packaging/apt"
do
  require_text "$design" "$text"
done

require_text "docs/package-manager-roadmap.md" "apt stays parked until signing, repository metadata, hosting, key rotation, and recovery are designed."
require_text "docs/distribution-trust.md" "apt remains parked because a repository is a stronger trust commitment"
require_text "docs/roadmap.md" "Design signed apt repository metadata before documenting apt repository"

require_absent_path "packaging/apt/Release"
require_absent_path "packaging/apt/InRelease"
require_absent_path "packaging/apt/Packages"
require_absent_path "packaging/apt/Sources"
require_absent_path "packaging/apt/Release.gpg"

printf 'apt repository design validation succeeded\n'
