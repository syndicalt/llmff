#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
usage:
  scripts/smoke-windows-msi.sh --msi <path>
  scripts/smoke-windows-msi.sh --payload-root <path>

Verifies a Windows llmff MSI payload. Use --msi on Windows hosts to extract
the installer with msiexec administrative install mode. Use --payload-root to
validate an already-staged payload root on any development host.
USAGE
}

msi=""
payload_root=""

while [ "$#" -gt 0 ]; do
  case "$1" in
    --msi)
      if [ "$#" -lt 2 ] || [ -z "$2" ]; then
        usage >&2
        exit 2
      fi
      msi="$2"
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

if [ -z "$msi" ] && [ -z "$payload_root" ]; then
  usage >&2
  exit 2
fi

if [ -n "$msi" ] && [ -n "$payload_root" ]; then
  printf 'error: choose either --msi or --payload-root, not both\n' >&2
  exit 2
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
work="$(mktemp -d)"
cleanup() {
  rm -rf "$work"
}
trap cleanup EXIT

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

if [ -n "$msi" ]; then
  if [ ! -f "$msi" ]; then
    printf 'error: Windows MSI not found: %s\n' "$msi" >&2
    exit 1
  fi

  case "$(uname -s)" in
    MINGW* | MSYS* | CYGWIN*)
      ;;
    *)
      printf 'error: Windows MSI smoke requires a Windows host; use --payload-root for local payload validation\n' >&2
      exit 1
      ;;
  esac

  if command -v msiexec.exe >/dev/null 2>&1; then
    msiexec_cmd="msiexec.exe"
  elif command -v msiexec >/dev/null 2>&1; then
    msiexec_cmd="msiexec"
  else
    printf 'error: msiexec is required to smoke test Windows MSI installers\n' >&2
    exit 1
  fi

  payload_root="$work/msi-root"
  msiexec_log="$work/msiexec.log"
  mkdir -p "$payload_root"
  msiexec_args=(
    /a "$(native_path "$msi")"
    /qn
    /l*v "$(native_path "$msiexec_log")"
    TARGETDIR="$(native_path "$payload_root")"
  )
  if command -v timeout >/dev/null 2>&1; then
    set +e
    timeout 180 "$msiexec_cmd" "${msiexec_args[@]}"
    status=$?
    set -e
    if [ "$status" -ne 0 ]; then
      printf 'error: msiexec administrative install failed or timed out with status %s\n' "$status" >&2
      if [ -f "$msiexec_log" ]; then
        tail -80 "$msiexec_log" >&2
      fi
      exit 1
    fi
  else
    set +e
    "$msiexec_cmd" "${msiexec_args[@]}"
    status=$?
    set -e
    if [ "$status" -ne 0 ]; then
      printf 'error: msiexec administrative install failed with status %s\n' "$status" >&2
      if [ -f "$msiexec_log" ]; then
        tail -80 "$msiexec_log" >&2
      fi
      exit 1
    fi
  fi
else
  if [ ! -d "$payload_root" ]; then
    printf 'error: payload root not found: %s\n' "$payload_root" >&2
    exit 1
  fi
  payload_root="$(cd "$payload_root" && pwd -P)"
fi

binary="$(find "$payload_root" -type f -name llmff.exe -print -quit)"
if [ -z "$binary" ]; then
  binary="$(find "$payload_root" -type f -name llmff -perm -111 -print -quit)"
fi

if [ -z "$binary" ] || [ ! -f "$binary" ]; then
  printf 'error: Windows MSI payload does not contain llmff executable under: %s\n' "$payload_root" >&2
  exit 1
fi

if [ "${binary##*.}" != "exe" ] && [ ! -x "$binary" ]; then
  printf 'error: Windows MSI payload binary is not executable: %s\n' "$binary" >&2
  exit 1
fi

run_dir="$work/run"
mkdir -p "$run_dir"

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
  printf 'error: Windows MSI payload run wrote unexpected output: %s\n' "$(cat "$output")" >&2
  exit 1
fi

printf 'Windows MSI smoke succeeded\n'
