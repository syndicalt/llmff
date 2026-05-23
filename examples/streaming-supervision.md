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
Keep the CLI exit code:

```bash
set -o pipefail
if ! llmff run examples/json-repair.yaml --events - > /tmp/llmff-events.jsonl; then
  echo "llmff failed; inspect /tmp/llmff-events.jsonl" >&2
  exit 1
fi
```
