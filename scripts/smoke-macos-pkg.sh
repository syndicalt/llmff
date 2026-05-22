#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
usage:
  scripts/smoke-macos-pkg.sh --pkg <path>
  scripts/smoke-macos-pkg.sh --payload-root <path>

Verifies a macOS llmff installer payload without installing it. Use --pkg on
Darwin hosts to expand a built .pkg with pkgutil. Use --payload-root to validate
a staged package root on any development host.
USAGE
}

pkg=""
payload_root=""

while [ "$#" -gt 0 ]; do
  case "$1" in
    --pkg)
      if [ "$#" -lt 2 ] || [ -z "$2" ]; then
        usage >&2
        exit 2
      fi
      pkg="$2"
      shift 2
      ;;
    --payload-root)
      if [ "$#" -lt 2 ] || [ -z "$2" ]; then
        usage >&2
        exit 2
      fi
      payload_root="$2"
      shift 2
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      usage >&2
      exit 2
      ;;
  esac
done

if [ -z "$pkg" ] && [ -z "$payload_root" ]; then
  usage >&2
  exit 2
fi

if [ -n "$pkg" ] && [ -n "$payload_root" ]; then
  printf 'error: choose either --pkg or --payload-root, not both\n' >&2
  exit 2
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
work="$(mktemp -d)"
cleanup() {
  rm -rf "$work"
}
trap cleanup EXIT

if [ -n "$pkg" ]; then
  if [ ! -f "$pkg" ]; then
    printf 'error: macOS package not found: %s\n' "$pkg" >&2
    exit 1
  fi

  case "$(uname -s)" in
    Darwin)
      ;;
    *)
      printf 'error: macOS .pkg smoke requires a Darwin host; use --payload-root for local payload validation\n' >&2
      exit 1
      ;;
  esac

  if ! command -v pkgutil >/dev/null 2>&1; then
    printf 'error: pkgutil is required to smoke test macOS .pkg installers\n' >&2
    exit 1
  fi

  expanded="$work/expanded"
  pkgutil --expand-full "$pkg" "$expanded"
  payload_root="$(find "$expanded" -path '*/Payload/usr/local/bin/llmff' -type f -perm -111 -print -quit)"
  if [ -z "$payload_root" ]; then
    printf 'error: expanded macOS package does not contain executable usr/local/bin/llmff: %s\n' "$pkg" >&2
    exit 1
  fi
  payload_root="${payload_root%/usr/local/bin/llmff}"
else
  if [ ! -d "$payload_root" ]; then
    printf 'error: payload root not found: %s\n' "$payload_root" >&2
    exit 1
  fi
  payload_root="$(cd "$payload_root" && pwd -P)"
fi

binary="$payload_root/usr/local/bin/llmff"
if [ ! -x "$binary" ]; then
  printf 'error: payload llmff binary is missing or not executable: %s\n' "$binary" >&2
  exit 1
fi

run_dir="$work/run"
mkdir -p "$run_dir"

"$binary" --version
"$binary" stages list | grep '^infer$'
"$binary" inspect "$repo_root/examples/json-repair.yaml"

prompt="$run_dir/question.txt"
output="$run_dir/answer.json"
manifest="$run_dir/pipeline.yaml"

printf 'Return an answer object\n' >"$prompt"
cat >"$manifest" <<YAML
version: 1
inputs:
  prompt:
    path: "$prompt"
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: load_prompt
    model: mock:good
outputs:
  final:
    from: draft
    path: "$output"
YAML

LLMFF_MOCK_GOOD_RESPONSE='{"answer":"ok"}' "$binary" run "$manifest"

if [ "$(cat "$output")" != '{"answer":"ok"}' ]; then
  printf 'error: macOS package payload run wrote unexpected output: %s\n' "$(cat "$output")" >&2
  exit 1
fi

printf 'macOS package smoke succeeded\n'
