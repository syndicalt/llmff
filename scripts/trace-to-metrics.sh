#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
  printf 'usage: %s <trace-jsonl>\n' "$0" >&2
  exit 2
fi

python3 - "$1" <<'PY'
import json
import re
import sys
from pathlib import Path


LABEL_SAFE = re.compile(r"[^A-Za-z0-9_:-]")


def label(value):
    return LABEL_SAFE.sub("_", str(value))


def read_events(path):
    events = []
    for line_number, line in enumerate(Path(path).read_text().splitlines(), start=1):
        if not line.strip():
            continue
        try:
            events.append(json.loads(line))
        except json.JSONDecodeError as error:
            raise SystemExit(f"invalid trace JSON on line {line_number}: {error}") from error
    return events


events = read_events(sys.argv[1])
run_started = next((event for event in events if event.get("event") == "run_started"), {})
stage_events = [event for event in events if event.get("event") == "stage_finished"]
run_events = [
    event for event in events if event.get("event") in {"run_finished", "run_failed"}
]
last_run = run_events[-1] if run_events else {}


def timestamp_ms(event):
    value = event.get("timestamp_ms")
    return value if isinstance(value, int) else None


duration_total = sum(int(event.get("duration_ms") or 0) for event in stage_events)
run_start_ms = timestamp_ms(run_started)
run_end_ms = timestamp_ms(last_run)
run_wall_ms = (
    max(0, run_end_ms - run_start_ms)
    if run_start_ms is not None and run_end_ms is not None
    else 0
)
prompt_tokens = sum(int(event.get("prompt_tokens") or 0) for event in stage_events)
completion_tokens = sum(int(event.get("completion_tokens") or 0) for event in stage_events)
total_tokens = sum(int(event.get("total_tokens") or 0) for event in stage_events)
cache_events = [event for event in stage_events if "cache_hit" in event]
cache_hits = sum(1 for event in cache_events if event.get("cache_hit") is True)
cache_misses = sum(1 for event in cache_events if event.get("cache_hit") is False)
cache_total = cache_hits + cache_misses
cache_rate = (cache_hits / cache_total) if cache_total else 0.0
backend_errors = sum(
    1
    for event in events
    if event.get("event") == "run_failed" and event.get("failure_kind") == "backend"
)
backend_error_rate = (backend_errors / len(run_events)) if run_events else 0.0
failure_events = [event for event in events if event.get("event") == "run_failed"]
failures_total = len(failure_events)
failure_rate = (failures_total / len(run_events)) if run_events else 0.0
timeout_errors = sum(
    1 for event in failure_events if event.get("failure_kind") == "timeout"
)
timeout_error_rate = (timeout_errors / len(run_events)) if run_events else 0.0

print("# TYPE llmff_run_duration_ms gauge")
print(f"llmff_run_duration_ms {run_wall_ms}")
print("# TYPE llmff_stage_duration_ms_sum counter")
print(f"llmff_stage_duration_ms_sum {duration_total}")
print("# TYPE llmff_stage_duration_ms gauge")
for event in stage_events:
    print(
        'llmff_stage_duration_ms{stage_id="'
        f'{label(event.get("stage_id", "unknown"))}",op="{label(event.get("op", "unknown"))}"'
        f'}} {int(event.get("duration_ms") or 0)}'
    )
print("# TYPE llmff_tokens_total counter")
print(f"llmff_prompt_tokens_total {prompt_tokens}")
print(f"llmff_completion_tokens_total {completion_tokens}")
print(f"llmff_tokens_total {total_tokens}")
print("# TYPE llmff_cache_hit_rate gauge")
print(f"llmff_cache_hits_total {cache_hits}")
print(f"llmff_cache_misses_total {cache_misses}")
print(f"llmff_cache_hit_rate {cache_rate:.4f}")
print("# TYPE llmff_backend_error_rate gauge")
print(f"llmff_backend_errors_total {backend_errors}")
print(f"llmff_backend_error_rate {backend_error_rate:.4f}")
print("# TYPE llmff_failure_rate gauge")
print(f"llmff_failures_total {failures_total}")
print(f"llmff_failure_rate {failure_rate:.4f}")
print("# TYPE llmff_timeout_error_rate gauge")
print(f"llmff_timeout_errors_total {timeout_errors}")
print(f"llmff_timeout_error_rate {timeout_error_rate:.4f}")
PY
