# Supervisor Example

This supervisor pattern watches the live event stream, preserves the process
exit status, and exports a local summary after the run finishes.

```bash
#!/usr/bin/env bash
set -euo pipefail

trace=${1:-/tmp/llmff-trace.jsonl}
events=${2:-/tmp/llmff-events.jsonl}
rm -f "$trace" "$events"

set +e
llmff run examples/json-repair.yaml --trace "$trace" --events "$events"
status=$?
set -e

scripts/trace-to-summary.sh "$trace"
scripts/trace-to-metrics.sh "$trace" > /tmp/llmff-metrics.prom

if [ "$status" -ne 0 ]; then
  python3 -c 'import json,sys; [print(e.get("failure_kind","unknown"), e.get("failure_message","")) for e in map(json.loads, open(sys.argv[1])) if e.get("event") == "run_failed"]' "$events" >&2
fi

exit "$status"
```

Supervisors should treat the trace and event files as safe metadata streams.
They should not infer success from events alone; the process exit code remains
the final authority.
