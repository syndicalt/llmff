#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
usage:
  scripts/smoke-deb.sh --deb <path>

Extracts a llmff Debian package without root and verifies the packaged binary.
USAGE
}

deb=""

while [ "$#" -gt 0 ]; do
  case "$1" in
    --deb)
      if [ "$#" -lt 2 ] || [ -z "$2" ]; then
        usage >&2
        exit 2
      fi
      deb="$2"
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

if [ -z "$deb" ]; then
  usage >&2
  exit 2
fi

if [ ! -f "$deb" ]; then
  printf 'error: Debian package not found: %s\n' "$deb" >&2
  exit 1
fi

if ! command -v dpkg-deb >/dev/null 2>&1; then
  printf 'error: dpkg-deb is required to smoke test .deb packages\n' >&2
  exit 1
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
deb="$(cd "$(dirname "$deb")" && pwd -P)/$(basename "$deb")"

work="$(mktemp -d)"
cleanup() {
  rm -rf "$work"
}
trap cleanup EXIT

extract="$work/extract"
run_dir="$work/run"
mkdir -p "$extract" "$run_dir"

dpkg-deb -x "$deb" "$extract"
binary="$extract/usr/bin/llmff"

if [ ! -x "$binary" ]; then
  printf 'error: extracted llmff binary is missing or not executable: %s\n' "$binary" >&2
  exit 1
fi

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
    path: $prompt
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
    path: $output
YAML

LLMFF_MOCK_GOOD_RESPONSE='{"answer":"ok"}' "$binary" run "$manifest"

if [ "$(cat "$output")" != '{"answer":"ok"}' ]; then
  printf 'error: packaged llmff run wrote unexpected output: %s\n' "$(cat "$output")" >&2
  exit 1
fi

printf 'deb smoke succeeded\n'
