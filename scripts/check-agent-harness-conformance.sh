#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$repo_root"

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

manifest="$tmp_dir/pipeline.yaml"
input="$tmp_dir/question.txt"
output="$tmp_dir/answer.json"
run_dir="$tmp_dir/run"

printf 'Return an answer object\n' >"$input"
cat >"$manifest" <<YAML
version: 1
inputs:
  prompt:
    path: "$input"
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
    path: "$output"
YAML

LLMFF_MOCK_GOOD_RESPONSE='{"answer":"ok"}' \
  cargo run -q -p llmff -- run --run-dir "$run_dir" "$manifest"

python3 - "$run_dir" <<'PY'
import json
import sys
from pathlib import Path

run_dir = Path(sys.argv[1])
required = [
    "inspect.json",
    "trace.jsonl",
    "events.jsonl",
    "checkpoint.json",
    "result.json",
]
missing = [name for name in required if not (run_dir / name).is_file()]
if missing:
    raise SystemExit(f"missing run-dir artifacts: {', '.join(missing)}")

result = json.loads((run_dir / "result.json").read_text(encoding="utf-8"))
assert result["schema_version"] == 1
assert result["status"] == "succeeded"
assert result["exit_code"] == 0
assert result["failure"] is None
assert result["artifacts"] == {
    "inspect": "inspect.json",
    "trace": "trace.jsonl",
    "events": "events.jsonl",
    "checkpoint": "checkpoint.json",
}
assert result["manifest"]["hash"].startswith("sha256:")

inspect = json.loads((run_dir / "inspect.json").read_text(encoding="utf-8"))
assert inspect["format_version"] == 1
assert inspect["execution"]["artifacts"]["trace"].endswith("trace.jsonl")
assert inspect["execution"]["artifacts"]["events"].endswith("events.jsonl")

trace = (run_dir / "trace.jsonl").read_text(encoding="utf-8")
events = (run_dir / "events.jsonl").read_text(encoding="utf-8")
assert '"event":"run_started"' in trace
assert '"event":"run_finished"' in trace
assert '"event":"run_started"' in events
assert '"event":"run_finished"' in events
PY

fail_tool="$tmp_dir/fail-tool"
fail_manifest="$tmp_dir/fail.yaml"
fail_run_dir="$tmp_dir/fail-run"
cat >"$fail_tool" <<'SH'
#!/usr/bin/env sh
cat >/dev/null
printf 'tool failed\n' >&2
exit 7
SH
chmod +x "$fail_tool"
cat >"$fail_manifest" <<YAML
version: 1
inputs:
  prompt:
    path: "-"
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: call_tool
    op: tool
    from: load_prompt
    command: ["$fail_tool"]
outputs:
  final:
    from: call_tool
    path: "-"
YAML

set +e
printf 'payload\n' | cargo run -q -p llmff -- run --run-dir "$fail_run_dir" "$fail_manifest" >/dev/null 2>"$tmp_dir/fail.stderr"
status="$?"
set -e
if [ "$status" -ne 20 ]; then
  printf 'expected failing conformance run to exit 20, got %s\n' "$status" >&2
  cat "$tmp_dir/fail.stderr" >&2
  exit 1
fi

python3 - "$fail_run_dir" <<'PY'
import json
import sys
from pathlib import Path

run_dir = Path(sys.argv[1])
result = json.loads((run_dir / "result.json").read_text(encoding="utf-8"))
assert result["schema_version"] == 1
assert result["status"] == "failed"
assert result["exit_code"] == 20
assert result["failure"]["kind"] == "stage_execution"
assert result["failure"]["retry_recommendation"] == "check_stage_or_input"
assert (run_dir / "events.jsonl").is_file()
assert (run_dir / "trace.jsonl").is_file()
assert '"event":"run_failed"' in (run_dir / "events.jsonl").read_text(encoding="utf-8")
PY

printf 'agent harness conformance validation succeeded\n'
