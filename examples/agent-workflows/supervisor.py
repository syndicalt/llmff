#!/usr/bin/env python3
"""Run llmff as a supervised subprocess for agent workflows."""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def copy_fixture(work_dir: Path) -> Path:
    source_dir = repo_root() / "examples"
    for name in [
        "json-repair.yaml",
        "question.txt",
        "prompt.tmpl",
        "policy.md",
        "answer.schema.json",
    ]:
        shutil.copy2(source_dir / name, work_dir / name)
    return work_dir / "json-repair.yaml"


def run_pipeline(work_dir: Path) -> int:
    manifest = copy_fixture(work_dir)
    trace = work_dir / "trace.jsonl"
    checkpoint = work_dir / "checkpoint.json"
    llmff = os.environ.get("LLMFF_BIN", "llmff")

    env = os.environ.copy()
    env.setdefault("LLMFF_MOCK_BAD_RESPONSE", '{"wrong":true}')
    env.setdefault("LLMFF_MOCK_GOOD_RESPONSE", '{"answer":"ok"}')

    completed = subprocess.run(
        [
            llmff,
            "run",
            str(manifest),
            "--events",
            "-",
            "--trace",
            str(trace),
            "--checkpoint",
            str(checkpoint),
            "--timeout-ms",
            "30000",
        ],
        cwd=work_dir,
        env=env,
        text=True,
        capture_output=True,
        check=False,
    )

    events = [
        json.loads(line)
        for line in completed.stdout.splitlines()
        if line.strip().startswith("{")
    ]
    failures = [event for event in events if event.get("event") == "run_failed"]
    status = "ok" if completed.returncode == 0 else "failed"

    print(f"run_status={status}")
    print(f"event_count={len(events)}")
    print(f"trace={trace}")
    print(f"checkpoint={checkpoint}")
    print(f"output={work_dir / 'answer.json'}")

    if failures:
        failure = failures[-1]
        print(
            "failure_kind="
            f"{failure.get('failure_kind', 'unknown')} "
            f"failure_message={failure.get('failure_message', '')}",
            file=sys.stderr,
        )

    if completed.stderr:
        print(completed.stderr, file=sys.stderr, end="")

    return completed.returncode


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--work-dir",
        type=Path,
        help="Directory for copied fixtures, trace, checkpoint, and output.",
    )
    args = parser.parse_args()

    if args.work_dir:
        args.work_dir.mkdir(parents=True, exist_ok=True)
        return run_pipeline(args.work_dir)

    with tempfile.TemporaryDirectory(prefix="llmff-agent-") as temp:
        return run_pipeline(Path(temp))


if __name__ == "__main__":
    raise SystemExit(main())
