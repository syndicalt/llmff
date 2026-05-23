#!/usr/bin/env bash
set -euo pipefail

binary="${LLMFF_BIN:-llmff}"
tmp="$(mktemp -d)"
server_pid=""

cleanup() {
  if [[ -n "$server_pid" ]]; then
    kill "$server_pid" >/dev/null 2>&1 || true
    wait "$server_pid" >/dev/null 2>&1 || true
  fi
  rm -rf "$tmp"
}
trap cleanup EXIT

prompt="$tmp/question.txt"
mock_manifest="$tmp/mock.yaml"
mock_output="$tmp/mock-output.txt"
stream_manifest="$tmp/stream.yaml"
stream_output="$tmp/stream-output.txt"

printf 'Say hello from llmff.\n' > "$prompt"

cat > "$mock_manifest" <<EOF
version: 1
inputs:
  prompt:
    path: $prompt
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
    path: $mock_output
EOF

cat > "$stream_manifest" <<EOF
version: 1
inputs:
  prompt:
    path: $prompt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: load_prompt
    model: local:test-model
outputs:
  final:
    from: draft
    path: $stream_output
EOF

validate_events() {
  local path="$1"
  python3 - "$path" <<'PY'
import json
import sys

path = sys.argv[1]
events = []
with open(path, "r", encoding="utf-8") as handle:
    for line_number, line in enumerate(handle, start=1):
        if not line.strip():
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError as error:
            raise SystemExit(f"{path}:{line_number}: invalid JSON: {error}") from error
        if "run_id" not in event or "event" not in event or "timestamp_ms" not in event:
            raise SystemExit(f"{path}:{line_number}: missing required event fields")
        events.append(event["event"])

expected = {"run_started", "stage_started", "stage_finished", "run_finished"}
missing = expected.difference(events)
if missing:
    raise SystemExit(f"{path}: missing events: {', '.join(sorted(missing))}")
PY
}

LLMFF_MOCK_GOOD_RESPONSE='mock answer' \
  "$binary" run --events - "$mock_manifest" > "$tmp/mock-events-stdout.jsonl"
validate_events "$tmp/mock-events-stdout.jsonl"
if grep -Fq 'mock answer' "$tmp/mock-events-stdout.jsonl"; then
  echo "payload output leaked into --events - stream" >&2
  exit 1
fi

LLMFF_MOCK_GOOD_RESPONSE='mock answer' \
  "$binary" run --events "$tmp/mock-events-file.jsonl" "$mock_manifest" > "$tmp/mock-events-file.stdout"
validate_events "$tmp/mock-events-file.jsonl"
if [[ -s "$tmp/mock-events-file.stdout" ]]; then
  echo "--events <path> wrote unexpected stdout" >&2
  exit 1
fi

LLMFF_MOCK_GOOD_RESPONSE='mock answer' \
  "$binary" run --stream-stage draft "$mock_manifest" > "$tmp/mock-stream.txt"
if [[ "$(cat "$tmp/mock-stream.txt")" != "mock answer" ]]; then
  echo "mock --stream-stage did not emit the selected stage payload" >&2
  exit 1
fi

cat > "$tmp/openai_stream_server.py" <<'PY'
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import pathlib
import sys

port_file = pathlib.Path(sys.argv[1])

class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        length = int(self.headers.get("content-length", "0"))
        body = json.loads(self.rfile.read(length))
        if self.path != "/v1/chat/completions":
            self.send_error(404)
            return
        if body.get("stream") is not True:
            self.send_error(400, "expected streaming request")
            return
        self.send_response(200)
        self.send_header("content-type", "text/event-stream")
        self.end_headers()
        for payload in (
            {"choices": [{"delta": {"content": "stream "}}]},
            {"choices": [{"delta": {"content": "answer"}}]},
            {"choices": [{"delta": {}}], "usage": {"prompt_tokens": 2, "completion_tokens": 2, "total_tokens": 4}},
        ):
            self.wfile.write(f"data: {json.dumps(payload)}\n\n".encode("utf-8"))
            self.wfile.flush()
        self.wfile.write(b"data: [DONE]\n\n")
        self.wfile.flush()

    def log_message(self, format, *args):
        return

server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
port_file.write_text(str(server.server_address[1]), encoding="utf-8")
server.serve_forever()
PY

python3 "$tmp/openai_stream_server.py" "$tmp/openai-port" &
server_pid="$!"
for _ in $(seq 1 50); do
  [[ -s "$tmp/openai-port" ]] && break
  sleep 0.1
done
if [[ ! -s "$tmp/openai-port" ]]; then
  echo "streaming backend fixture did not start" >&2
  exit 1
fi

port="$(cat "$tmp/openai-port")"
"$binary" run \
  --backend "local=http://127.0.0.1:$port" \
  --api-key local=fixture-key \
  --events "$tmp/stream-events-file.jsonl" \
  --stream-stage draft \
  "$stream_manifest" > "$tmp/stream-stage.txt"
validate_events "$tmp/stream-events-file.jsonl"
if [[ "$(cat "$tmp/stream-stage.txt")" != "stream answer" ]]; then
  echo "streaming backend --stream-stage did not emit streamed deltas" >&2
  exit 1
fi
if ! grep -Fq '"total_tokens":4' "$tmp/stream-events-file.jsonl"; then
  echo "streaming backend event file did not include usage metadata" >&2
  exit 1
fi

echo "events and streaming smoke passed"
