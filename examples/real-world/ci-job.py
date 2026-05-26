#!/usr/bin/env python3
"""Run a CI-style llmff gate with inspect, artifacts, and exit-code authority."""

from __future__ import annotations

import argparse
import json
import os
import sys
import tempfile
from pathlib import Path

from workflow_support import (
    MOCK_TRIAGE_RESPONSE,
    print_inspect_summary,
    print_process_output,
    read_json,
    run_llmff,
    write_issue_manifest,
)


def run_ci_job(work_dir: Path) -> int:
    work_dir.mkdir(parents=True, exist_ok=True)
    run_dir = work_dir / "llmff-run"
    output = work_dir / "issue-triage.json"
    manifest = write_issue_manifest(work_dir, output_path=output.name)
    inspect_path = work_dir / "inspect.json"

    env = os.environ.copy()
    env["LLMFF_MOCK_GOOD_RESPONSE"] = MOCK_TRIAGE_RESPONSE

    inspect = run_llmff(
        ["inspect", str(manifest), "--format", "json"],
        cwd=work_dir,
        env=env,
    )
    if inspect.returncode != 0:
        print_process_output(inspect)
        return inspect.returncode

    inspect_path.write_text(inspect.stdout, encoding="utf-8")
    inspect_report = json.loads(inspect.stdout)

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

    print("workflow=ci")
    print_inspect_summary(inspect_report)
    print(f"run_dir={run_dir}")
    print(f"output={output}")
    print(f"exit_code={completed.returncode}")
    print(f"result_status={result.get('status', 'missing')}")

    if completed.returncode != 0:
        print_process_output(completed)
        return completed.returncode

    if not output.exists():
        print(f"declared output artifact missing: {output}", file=sys.stderr)
        return 1

    print("ci_status=passed")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--work-dir", type=Path)
    args = parser.parse_args()

    if args.work_dir:
        return run_ci_job(args.work_dir)

    with tempfile.TemporaryDirectory(prefix="llmff-ci-job-") as temp:
        return run_ci_job(Path(temp))


if __name__ == "__main__":
    raise SystemExit(main())
