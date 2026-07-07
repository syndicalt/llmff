# Shared helpers for scripts/check-*.sh gates.
#
# Source with the path-robust pattern:
#   SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
#   . "$SCRIPT_DIR/lib/checks.sh"
#
# Every helper prints a script-actionable "error: ..." line to stderr and
# exits 1 on failure -- the same shape every check-*.sh script used to
# implement individually. Callers may set REQUIRE_FILE_LABEL before calling
# require_file (or anything that calls it, e.g. require_text/require_pattern)
# to customize the "missing" message; it defaults to "missing required file".
# This file assumes the sourcing script already ran `set -euo pipefail`.

require_file() {
  local path="$1"
  local label="${REQUIRE_FILE_LABEL:-missing required file}"
  if [ ! -f "$path" ]; then
    printf 'error: %s: %s\n' "$label" "$path" >&2
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

# require_pattern <path> <grep-E-pattern> [description]
#
# Structural counterpart to require_text for assertions that must hold for a
# *shape* of value (a 40-hex commit SHA, a numeric CI run id, ...) rather than
# a pinned literal. Used to de-ceremony point-in-time value pins while
# keeping the verification intent.
require_pattern() {
  local path="$1"
  local pattern="$2"
  local description="${3:-must match pattern}"
  require_file "$path"
  if ! grep -Eq -- "$pattern" "$path"; then
    printf 'error: %s %s: %s\n' "$path" "$description" "$pattern" >&2
    exit 1
  fi
}

# require_release_commit_matches_tag <evidence-path> <tag>
#
# Verifies the evidence file records a 40-hex release commit and, when the
# release tag is resolvable in this checkout, that the recorded commit is the
# commit the tag actually points to. Ground-truth verification against git
# replaces pinned SHA copies; shallow checkouts without tags still get the
# shape check.
require_release_commit_matches_tag() {
  local path="$1"
  local tag="$2"
  local recorded actual
  require_pattern "$path" 'Release commit: `[0-9a-f]{40}`' \
    "must record a 40-hex commit SHA"
  recorded="$(grep -Eo 'Release commit: `[0-9a-f]{40}`' "$path" | head -n 1 | grep -Eo '[0-9a-f]{40}')"
  if actual="$(git rev-list -n 1 "$tag" 2>/dev/null)"; then
    if [ "$recorded" != "$actual" ]; then
      printf 'error: %s records release commit %s but tag %s points to %s\n' \
        "$path" "$recorded" "$tag" "$actual" >&2
      exit 1
    fi
  fi
}
