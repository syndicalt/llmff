#!/usr/bin/env python3
"""Run a scheduled-job style llmff task and record scheduler state."""

from __future__ import annotations

import argparse
import json
import os
import sys
import tempfile
from pathlib import Path

from workflow_support import (
    MOCK_MEETING_RESPONSE,
    print_inspect_summary,
    print_process_output,
    read_json,
    run_llmff,
    write_meeting_manifest,
)


def run_scheduled_job(work_dir: Path) -> int:
    work_dir.mkdir(parents=True, exist_ok=True)
    run_dir = work_dir / "llmff-run"
    output = work_dir / "meeting-notes.json"
    state_file = work_dir / "last-success.json"
    manifest = write_meeting_manifest(work_dir, output_path=output.name)

    env = os.environ.copy()
    env["LLMFF_MOCK_GOOD_RESPONSE"] = MOCK_MEETING_RESPONSE

    inspect = run_llmff(
        ["inspect", str(manifest), "--format", "json"],
        cwd=work_dir,
        env=env,
    )
    if inspect.returncode != 0:
        print_process_output(inspect)
        return inspect.returncode

    completed = run_llmff(
        [
            "run",
            "--run-dir",
            str(run_dir),
            str(manifest),
            "--timeout-ms",
            "30000",
        ],
        cwd=work_dir,
        env=env,
    )
    result_path = run_dir / "result.json"
    result = read_json(result_path) if result_path.exists() else {}

    print("workflow=scheduled-job")
    print("schedule_id=nightly-meeting-notes")
    print_inspect_summary(json.loads(inspect.stdout))
    print(f"run_dir={run_dir}")
    print(f"exit_code={completed.returncode}")
    print(f"result_status={result.get('status', 'missing')}")

    if completed.returncode != 0:
        print("next_action=retry_next_window")
        print_process_output(completed)
        return completed.returncode

    if not output.exists():
        print(f"declared output artifact missing: {output}", file=sys.stderr)
        return 1

    state_file.write_text(
        json.dumps(
            {
                "schedule_id": "nightly-meeting-notes",
                "manifest_hash": json.loads(inspect.stdout)["manifest"]["hash"],
                "run_dir": str(run_dir),
                "output": str(output),
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    print(f"state_file={state_file}")
    print("next_action=record_success")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--work-dir", type=Path)
    args = parser.parse_args()

    if args.work_dir:
        return run_scheduled_job(args.work_dir)

    with tempfile.TemporaryDirectory(prefix="llmff-scheduled-job-") as temp:
        return run_scheduled_job(Path(temp))


if __name__ == "__main__":
    raise SystemExit(main())
