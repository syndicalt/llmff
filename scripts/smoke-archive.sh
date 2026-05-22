#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
usage:
  scripts/smoke-archive.sh --archive <path>

Extracts a llmff release archive without installing it and verifies the
packaged binary.
USAGE
}

archive=""

while [ "$#" -gt 0 ]; do
  case "$1" in
    --archive)
      if [ "$#" -lt 2 ] || [ -z "$2" ]; then
        usage >&2
        exit 2
      fi
      archive="$2"
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

if [ -z "$archive" ]; then
  usage >&2
  exit 2
fi

if [ ! -f "$archive" ]; then
  printf 'error: archive not found: %s\n' "$archive" >&2
  exit 1
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
archive="$(cd "$(dirname "$archive")" && pwd -P)/$(basename "$archive")"

work="$(mktemp -d)"
cleanup() {
  rm -rf "$work"
}
trap cleanup EXIT

extract="$work/extract"
run_dir="$work/run"
mkdir -p "$extract" "$run_dir"

native_path() {
  if command -v cygpath >/dev/null 2>&1; then
    cygpath -w "$1"
  else
    printf '%s\n' "$1"
  fi
}

yaml_quote() {
  printf "'%s'\n" "$(printf '%s' "$1" | sed "s/'/''/g")"
}

case "$archive" in
  *.tar.gz | *.tgz)
    tar -C "$extract" -xzf "$archive"
    ;;
  *.zip)
    if command -v unzip >/dev/null 2>&1; then
      unzip -q "$archive" -d "$extract"
    elif command -v powershell.exe >/dev/null 2>&1; then
      powershell.exe -NoProfile -Command \
        "Expand-Archive -LiteralPath '$(native_path "$archive")' -DestinationPath '$(native_path "$extract")' -Force" >/dev/null
    elif command -v 7z >/dev/null 2>&1; then
      7z x "$archive" "-o$extract" >/dev/null
    else
      printf 'error: unzip, powershell.exe, or 7z is required to smoke test zip archives\n' >&2
      exit 1
    fi
    ;;
  *)
    printf 'error: unsupported archive type: %s\n' "$archive" >&2
    exit 1
    ;;
esac

binary="$(find "$extract" -mindepth 2 -maxdepth 2 -type f -name llmff -perm -111 -print -quit)"
if [ -z "$binary" ]; then
  binary="$(find "$extract" -mindepth 2 -maxdepth 2 -type f -name llmff.exe -print -quit)"
fi

if [ -z "$binary" ] || [ ! -f "$binary" ]; then
  printf 'error: extracted llmff binary is missing in archive: %s\n' "$archive" >&2
  exit 1
fi

if [ "${binary##*.}" != "exe" ] && [ ! -x "$binary" ]; then
  printf 'error: extracted llmff binary is not executable: %s\n' "$binary" >&2
  exit 1
fi

"$binary" --version
"$binary" stages list | grep '^infer$'
"$binary" inspect "$(native_path "$repo_root/examples/json-repair.yaml")"

prompt="$run_dir/question.txt"
output="$run_dir/answer.json"
manifest="$run_dir/pipeline.yaml"

printf 'Return an answer object\n' >"$prompt"
cat >"$manifest" <<YAML
version: 1
inputs:
  prompt:
    path: $(yaml_quote "$(native_path "$prompt")")
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
    path: $(yaml_quote "$(native_path "$output")")
YAML

LLMFF_MOCK_GOOD_RESPONSE='{"answer":"ok"}' "$binary" run "$(native_path "$manifest")"

if [ "$(cat "$output")" != '{"answer":"ok"}' ]; then
  printf 'error: archived llmff run wrote unexpected output: %s\n' "$(cat "$output")" >&2
  exit 1
fi

printf 'archive smoke succeeded\n'
