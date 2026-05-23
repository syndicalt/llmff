#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)
caller_dir=$(pwd -P)
llmff=${LLMFF_BIN:-llmff}
if [[ "$llmff" == */* && "$llmff" != /* ]]; then
  llmff="$caller_dir/$llmff"
fi

work_dir=""
while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --work-dir)
      if [[ "$#" -lt 2 ]]; then
        printf 'usage: %s [--work-dir <path>]\n' "$0" >&2
        exit 2
      fi
      work_dir=$2
      shift 2
      ;;
    -h | --help)
      printf 'usage: %s [--work-dir <path>]\n' "$0"
      exit 0
      ;;
    *)
      printf 'unknown argument: %s\n' "$1" >&2
      printf 'usage: %s [--work-dir <path>]\n' "$0" >&2
      exit 2
      ;;
  esac
done

if [[ -z "$work_dir" ]]; then
  work_dir=$(mktemp -d "${TMPDIR:-/tmp}/llmff-observability.XXXXXX")
else
  mkdir -p "$work_dir"
  work_dir=$(cd "$work_dir" && pwd -P)
fi

for file in json-repair.yaml question.txt prompt.tmpl policy.md answer.schema.json; do
  cp "$repo_root/examples/$file" "$work_dir/$file"
done

manifest="$work_dir/json-repair.yaml"
trace="$work_dir/trace.jsonl"
events="$work_dir/events.jsonl"
summary="$work_dir/summary.txt"
metrics="$work_dir/metrics.prom"
checkpoint="$work_dir/checkpoint.json"
live_summary="$work_dir/live-events.txt"
output="$work_dir/answer.json"

rm -f "$trace" "$events" "$summary" "$metrics" "$checkpoint" "$live_summary" "$output"

export LLMFF_MOCK_BAD_RESPONSE=${LLMFF_MOCK_BAD_RESPONSE:-'{"wrong":true}'}
export LLMFF_MOCK_GOOD_RESPONSE=${LLMFF_MOCK_GOOD_RESPONSE:-'{"answer":"ok"}'}

set +e
(
  cd "$work_dir"
  "$llmff" run "$manifest" --events - --trace "$trace" --checkpoint "$checkpoint"
) | tee "$events" | python3 -c '
import json
import sys

count = 0
failed = None
for line in sys.stdin:
    if not line.strip():
        continue
    event = json.loads(line)
    count += 1
    if event.get("event") == "run_failed":
        failed = event.get("failure_kind", "unknown")

print(f"live_event_count={count}")
if failed:
    print(f"failure_kind={failed}")
' > "$live_summary"
statuses=("${PIPESTATUS[@]}")
set -e

run_status=${statuses[0]}
cat "$live_summary"

if [[ "$run_status" -eq 0 ]]; then
  printf 'run_status=ok\n'
else
  printf 'run_status=failed exit_code=%s\n' "$run_status"
fi

"$repo_root/scripts/trace-to-summary.sh" "$trace" > "$summary"
"$repo_root/scripts/trace-to-metrics.sh" "$trace" > "$metrics"

summary_has_timing=false
if grep -Fq 'timing run_wall_ms=' "$summary"; then
  summary_has_timing=true
fi

metrics_has_run_duration=false
if grep -Fq 'llmff_run_duration_ms ' "$metrics"; then
  metrics_has_run_duration=true
fi

output_exists=false
if [[ -s "$output" ]]; then
  output_exists=true
fi

printf 'trace=%s\n' "$trace"
printf 'events=%s\n' "$events"
printf 'summary=%s\n' "$summary"
printf 'metrics=%s\n' "$metrics"
printf 'summary_has_timing=%s\n' "$summary_has_timing"
printf 'metrics_has_run_duration=%s\n' "$metrics_has_run_duration"
printf 'output_exists=%s\n' "$output_exists"

exit "$run_status"
