#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$repo_root"

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

if [ -z "${LLMFF_BIN:-}" ]; then
  cargo build -q -p llmff
  export LLMFF_BIN="$repo_root/target/debug/llmff"
fi

require_file() {
  local path="$1"
  if [ ! -f "$path" ]; then
    printf 'error: missing real-world workflow example: %s\n' "$path" >&2
    exit 1
  fi
}

require_text() {
  local path="$1"
  local text="$2"
  if ! grep -Fq -- "$text" "$path"; then
    printf 'error: %s must contain: %s\n' "$path" "$text" >&2
    printf '%s contents:\n' "$path" >&2
    cat "$path" >&2
    exit 1
  fi
}

run_example() {
  local name="$1"
  local script="$2"
  local output="$tmp_dir/$name.out"
  require_file "$script"
  python3 "$script" --work-dir "$tmp_dir/$name" >"$output"
  printf '%s\n' "$output"
}

ci_output="$(run_example ci examples/real-world/ci-job.py)"
require_text "$ci_output" "workflow=ci"
require_text "$ci_output" "ci_status=passed"
require_text "$ci_output" "exit_code=0"
require_text "$ci_output" "result_status=succeeded"

queue_output="$(run_example queue examples/real-world/queue-worker.py)"
require_text "$queue_output" "workflow=queue-worker"
require_text "$queue_output" "queue_processed=2"
require_text "$queue_output" "queue_failed=0"
require_text "$queue_output" "queue_ack_ticket-1001=true"
require_text "$queue_output" "queue_ack_ticket-1002=true"

scheduled_output="$(run_example scheduled examples/real-world/scheduled-job.py)"
require_text "$scheduled_output" "workflow=scheduled-job"
require_text "$scheduled_output" "exit_code=0"
require_text "$scheduled_output" "next_action=record_success"

failure_output="$(run_example failure examples/real-world/failure-triage.py)"
require_text "$failure_output" "workflow=failure-triage"
require_text "$failure_output" "llmff_exit_code=20"
require_text "$failure_output" "failure_kind=stage_execution"
require_text "$failure_output" "triage_decision=check_stage_or_input"

printf 'real-world workflow validation succeeded\n'
