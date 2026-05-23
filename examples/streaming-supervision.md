# Streaming Supervision Examples

These examples keep lifecycle events separate from stage payload output so shell
tools can process JSONL events without reading model text or final payloads.

## Pipe Events To Shell Tools

Use `--events -` when all pipeline outputs write to files:

```bash
LLMFF_MOCK_GOOD_RESPONSE='{"answer":"ok"}' \
llmff run examples/json-repair.yaml --events - \
  | tee /tmp/llmff-events.jsonl \
  | python3 -c 'import json,sys; [print(json.loads(line)["event"]) for line in sys.stdin if line.strip()]'
```

The command above sends only lifecycle events through the pipe. The example
manifest writes the final answer to `examples/answer.json`, so payload output
does not interleave with event JSONL.

## Stream One Stage And Save Events

Use `--events <path>` when `--stream-stage` owns stdout:

```bash
LLMFF_MOCK_GOOD_RESPONSE='streamed answer' \
llmff run -i examples/question.txt \
  -g 'load#prompt | infer#draft(model=mock:good) | write(path=/tmp/llmff-final.txt)' \
  --events /tmp/llmff-events.jsonl \
  --stream-stage draft \
  > /tmp/llmff-draft.txt
```

Inspect stage status without touching the streamed payload:

```bash
python3 -c 'import json,sys; [print(e["stage_id"], e.get("status")) for e in map(json.loads, open(sys.argv[1])) if e["event"] == "stage_finished"]' \
  /tmp/llmff-events.jsonl
```

## Watch For Failed Processes

Events are live progress records, not a replacement for process supervision.
`run_failed` gives supervisors a stable failure class when event output is
available, but the CLI exit code remains the final authority:

```bash
set -o pipefail
if ! llmff run examples/json-repair.yaml --events - > /tmp/llmff-events.jsonl; then
  python3 -c 'import json,sys; [print(e["failure_kind"], e["failure_message"]) for e in map(json.loads, open(sys.argv[1])) if e["event"] == "run_failed"]' \
    /tmp/llmff-events.jsonl >&2
  exit 1
fi
```

## Supervise Long-Running Runs

For a long-running pipeline, write events to a file and tail only the lifecycle
stream. Keep stdout free for the selected stage or final payload:

```bash
events=/tmp/llmff-events.jsonl
rm -f "$events"

llmff run examples/json-repair.yaml --events "$events" &
pid=$!

tail -n 0 -F "$events" \
  | python3 -c 'import json,sys; [print(e["event"], e.get("stage_id",""), e.get("status","")) for e in map(json.loads, sys.stdin)]' &
tail_pid=$!

wait "$pid"
status=$?
kill "$tail_pid" >/dev/null 2>&1 || true
exit "$status"
```

## Supervise Parallel Execution

Parallel runs can interleave independent stage events. Track stages by
`stage_id`, not by adjacent lines:

```bash
llmff run examples/json-repair.yaml --parallel --events - \
  | python3 -c 'import json,sys; state={}; [state.update({e["stage_id"]: e.get("status","started")}) or print(state) for e in map(json.loads, sys.stdin) if e.get("stage_id")]'
```

## Export A Post-Run Dashboard

Use `--trace` for post-run summaries and metrics. The exporters read only local
JSONL files and do not contact external services:

```bash
trace=/tmp/llmff-trace.jsonl
llmff run examples/json-repair.yaml --trace "$trace"

scripts/trace-to-summary.sh "$trace"
scripts/trace-to-metrics.sh "$trace" > /tmp/llmff-metrics.prom
```

The summary includes run wall-clock duration, stage timing, output artifact
locations, cache artifact locations, token usage, cache hit rate, timeout rate,
and failure breakdowns. See
`examples/supervision/dashboard.md` and
`examples/supervision/supervisor.md` for complete local patterns.
