#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
  printf 'usage: %s <trace-jsonl>\n' "$0" >&2
  exit 2
fi

python3 - "$1" <<'PY'
import json
import sys
from pathlib import Path


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
stage_events = [event for event in events if event.get("event") == "stage_finished"]
run_events = [
    event for event in events if event.get("event") in {"run_finished", "run_failed"}
]
last_run = run_events[-1] if run_events else {}

stage_total = len(stage_events)
stage_success = sum(1 for event in stage_events if event.get("status") == "success")
stage_failed = sum(1 for event in stage_events if event.get("status") not in {"success", "skipped"})
duration_total = sum(int(event.get("duration_ms") or 0) for event in stage_events)

prompt_tokens = sum(int(event.get("prompt_tokens") or 0) for event in stage_events)
completion_tokens = sum(int(event.get("completion_tokens") or 0) for event in stage_events)
total_tokens = sum(int(event.get("total_tokens") or 0) for event in stage_events)

cache_events = [event for event in stage_events if "cache_hit" in event]
cache_hits = sum(1 for event in cache_events if event.get("cache_hit") is True)
cache_misses = sum(1 for event in cache_events if event.get("cache_hit") is False)
cache_total = cache_hits + cache_misses
cache_rate = (cache_hits / cache_total * 100.0) if cache_total else 0.0

backend_errors = sum(
    1
    for event in events
    if event.get("event") == "run_failed" and event.get("failure_kind") == "backend"
)
run_total = len(run_events)
backend_error_rate = (backend_errors / run_total * 100.0) if run_total else 0.0
failure_events = [event for event in events if event.get("event") == "run_failed"]
timeout_errors = sum(
    1 for event in failure_events if event.get("failure_kind") == "timeout"
)
output_artifacts = [
    event
    for event in stage_events
    if isinstance(event.get("output_path"), str) and event.get("output_path")
]
cache_artifacts = [
    event
    for event in stage_events
    if isinstance(event.get("cache_path"), str) and event.get("cache_path")
]

print(f"run {last_run.get('run_id', 'unknown')} {last_run.get('status', 'unknown')}")
print(f"stages total={stage_total} success={stage_success} failed={stage_failed}")
print(f"timing total_stage_ms={duration_total}")
for event in stage_events:
    print(
        "stage "
        f"{event.get('stage_id', 'unknown')} "
        f"op={event.get('op', 'unknown')} "
        f"status={event.get('status', 'unknown')} "
        f"duration_ms={int(event.get('duration_ms') or 0)}"
    )
print(f"artifacts outputs={len(output_artifacts)} caches={len(cache_artifacts)}")
for event in output_artifacts:
    print(
        "artifact output "
        f"stage={event.get('stage_id', 'unknown')} "
        f"path={event.get('output_path')}"
    )
for event in cache_artifacts:
    cache_hit = str(event.get("cache_hit", "unknown")).lower()
    print(
        "artifact cache "
        f"stage={event.get('stage_id', 'unknown')} "
        f"path={event.get('cache_path')} "
        f"hit={cache_hit}"
    )
print(
    f"tokens prompt={prompt_tokens} completion={completion_tokens} total={total_tokens}"
)
print(f"cache hits={cache_hits} misses={cache_misses} hit_rate={cache_rate:.2f}%")
print(f"backend_errors total={backend_errors} rate={backend_error_rate:.2f}%")
print(
    f"failures total={len(failure_events)} backend={backend_errors} timeout={timeout_errors}"
)
for event in failure_events:
    print(
        "failure "
        f"kind={event.get('failure_kind', 'unknown')} "
        f"message={event.get('failure_message', '')}"
    )
PY
