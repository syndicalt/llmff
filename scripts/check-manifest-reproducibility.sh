#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$SCRIPT_DIR/lib/checks.sh"
REQUIRE_FILE_LABEL="missing manifest reproducibility artifact"

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$repo_root"

guide="docs/manifest-reproducibility.md"
schema="docs/schemas/inspect-report-v1.schema.json"
fixture="fixtures/golden/inspect/report.json"

require_file "$guide"
require_file "$schema"
require_file "$fixture"

for text in \
  "manifest hash" \
  "resolved inputs" \
  "resolved outputs" \
  "stage order" \
  "backend aliases" \
  "model ids" \
  "plugin dependencies" \
  "cache policy" \
  "checkpoint/resume policy" \
  "manifest lockfile remains parked" \
  "materially improves portability"
do
  require_text "$guide" "$text"
done

for text in \
  '"hash"' \
  '"compatibility"' \
  '"inputs"' \
  '"outputs"' \
  '"stage_order"' \
  '"backends"' \
  '"model"' \
  '"plugins"' \
  '"cache_policy"' \
  '"checkpoint"' \
  '"execution"'
do
  require_text "$schema" "$text"
  require_text "$fixture" "$text"
done

python3 - "$fixture" <<'PY'
import json
import sys
from pathlib import Path

fixture = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))

stage_order = fixture["stage_order"]
stage_ids = [stage["id"] for stage in fixture["stages"]]
if stage_ids != stage_order:
    raise SystemExit("inspect fixture stages must exactly match stage_order")

inputs = fixture["inputs"]
for required in ["prompt", "stdin", "batch"]:
    if required not in inputs:
        raise SystemExit(f"inspect fixture must include {required} input metadata")

outputs = fixture["outputs"]
if "final" not in outputs or "stdout" not in outputs:
    raise SystemExit("inspect fixture must include final and stdout output metadata")

if not any(stage.get("cache_policy") for stage in fixture["stages"]):
    raise SystemExit("inspect fixture must include cache policy metadata")
if not any(stage.get("timeout_ms") is not None for stage in fixture["stages"]):
    raise SystemExit("inspect fixture must include stage timeout metadata")
if not any(stage.get("retry") for stage in fixture["stages"]):
    raise SystemExit("inspect fixture must include stage retry metadata")
if not any(stage.get("plugin") for stage in fixture["stages"]):
    raise SystemExit("inspect fixture must include plugin dependency metadata")
if not any(stage.get("writes_stdout") for stage in fixture["stages"]):
    raise SystemExit("inspect fixture must include stdout-producing stage metadata")

execution = fixture["execution"]
stdout_owners = [
    execution["stdout"]["events"],
    execution["stdout"]["stream_stage"],
    execution["stdout"]["manifest_outputs"],
]
if sum(1 for owner in stdout_owners if owner) != 1:
    raise SystemExit("inspect fixture must show exactly one stdout owner")
if not execution["artifacts"]["trace"] or not execution["artifacts"]["events"]:
    raise SystemExit("inspect fixture must show trace and events artifact paths")
PY

require_text "docs/roadmap.md" "Explore lockfile or manifest-lock support only if it materially improves"
require_text "docs/roadmap.md" "Maintain schema compatibility fixtures for every additive manifest contract"

printf 'manifest reproducibility validation succeeded\n'
