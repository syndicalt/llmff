#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
usage:
  scripts/smoke-install.sh --path <repo-path>
  scripts/smoke-install.sh --git <repo-url> [--tag <tag>]
USAGE
}

if [ "$#" -ne 2 ] && [ "$#" -ne 4 ]; then
  usage >&2
  exit 2
fi

mode="$1"
source="$2"
tag=""
case "$mode" in
  --path | --git) ;;
  *)
    usage >&2
    exit 2
    ;;
esac

if [ "$#" -eq 4 ]; then
  if [ "$mode" != "--git" ] || [ "$3" != "--tag" ] || [ -z "$4" ]; then
    usage >&2
    exit 2
  fi
  tag="$4"
fi

tmp="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp"
}
trap cleanup EXIT

export CARGO_HOME="$tmp/cargo-home"
export CARGO_TARGET_DIR="$tmp/cargo-target"
export PATH="$CARGO_HOME/bin:$PATH"
mkdir -p "$CARGO_HOME" "$CARGO_TARGET_DIR"

if [ "$mode" = "--path" ]; then
  repo="$(cd "$source" && pwd -P)"
  cargo install --path "$repo/crates/llmff-cli" --locked
  example="$repo/examples/json-repair.yaml"
else
  install_args=(--git "$source")
  if [ -n "$tag" ]; then
    install_args+=(--tag "$tag")
  fi
  cargo install "${install_args[@]}" llmff --locked
  example="$tmp/json-repair.yaml"
  cat > "$example" <<'YAML'
version: 1
inputs:
  prompt:
    path: question.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: load_prompt
    model: mock:bad
  - id: validate
    op: validate_json
    from: draft
    schema: '{"type":"object","required":["answer"]}'
  - id: repair
    op: repair
    from: validate
    when: invalid
    model: mock:good
outputs:
  final:
    from: repair
    path: answer.json
YAML
  cat > "$tmp/question.txt" <<'TXT'
Return an answer object.
TXT
fi

llmff --version | grep 'llmff '
llmff stages list | grep '^infer$'
llmff inspect "$example"

run_dir="$tmp/run"
mkdir -p "$run_dir"
if [ "$mode" = "--path" ]; then
  cp "$(dirname "$example")"/* "$run_dir/"
  mv "$run_dir/json-repair.yaml" "$run_dir/pipeline.yaml"
else
  cp "$example" "$run_dir/pipeline.yaml"
  cp "$tmp/question.txt" "$run_dir/question.txt"
fi

(
  cd "$run_dir"
  LLMFF_MOCK_BAD_RESPONSE='{"wrong":true}' \
  LLMFF_MOCK_GOOD_RESPONSE='{"answer":"ok"}' \
    llmff run pipeline.yaml --trace trace.jsonl
  test "$(cat answer.json)" = '{"answer":"ok"}'
  llmff trace trace.jsonl | grep 'run cli-run succeeded'
)
