#!/usr/bin/env python3
"""Classify a failing llmff run by exit code and run_failed.failure_kind."""

from __future__ import annotations

import argparse
import os
import tempfile
from pathlib import Path

from workflow_support import (
    print_process_output,
    read_json,
    read_jsonl,
    run_llmff,
    write_issue_manifest,
)


def triage_decision(exit_code: int, failure_kind: str) -> str:
    if failure_kind in {"backend", "http", "timeout"} or exit_code == 21:
        return "retry_or_switch_backend"
    if failure_kind == "stage_execution" or exit_code == 20:
        return "check_stage_or_input"
    if exit_code == 10:
        return "fix_manifest_or_invocation"
    return "escalate"


def failure_kind_from_events(events_path: Path) -> str:
    failures = [
        event
        for event in read_jsonl(events_path)
        if event.get("event") == "run_failed" and event.get("failure_kind")
    ]
    if not failures:
        return "unknown"
    kind = failures[-1].get("failure_kind")
    return kind if isinstance(kind, str) else "unknown"


def run_failure_triage(work_dir: Path) -> int:
    work_dir.mkdir(parents=True, exist_ok=True)
    run_dir = work_dir / "llmff-run"
    manifest = write_issue_manifest(work_dir, output_path="bad-triage.json")

    env = os.environ.copy()
    env["LLMFF_MOCK_GOOD_RESPONSE"] = '{"wrong":true}'

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
    failure = result.get("failure") if isinstance(result.get("failure"), dict) else {}
    failure_kind = failure.get("kind") if isinstance(failure.get("kind"), str) else ""
    if not failure_kind:
        failure_kind = failure_kind_from_events(run_dir / "events.jsonl")

    print("workflow=failure-triage")
    print(f"run_dir={run_dir}")
    print(f"llmff_exit_code={completed.returncode}")
    print(f"result_status={result.get('status', 'missing')}")
    print(f"failure_kind={failure_kind}")
    print(f"triage_decision={triage_decision(completed.returncode, failure_kind)}")

    if completed.returncode != 20 or failure_kind != "stage_execution":
        print_process_output(completed)
        return 1

    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--work-dir", type=Path)
    args = parser.parse_args()

    if args.work_dir:
        return run_failure_triage(args.work_dir)

    with tempfile.TemporaryDirectory(prefix="llmff-failure-triage-") as temp:
        return run_failure_triage(Path(temp))


if __name__ == "__main__":
    raise SystemExit(main())
