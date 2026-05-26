#!/usr/bin/env python3
"""Run a queue-worker style batch classification job with per-item artifacts."""

from __future__ import annotations

import argparse
import json
import os
import sys
import tempfile
from pathlib import Path

from workflow_support import (
    MOCK_CLASSIFICATION_RESPONSE,
    print_inspect_summary,
    print_process_output,
    read_jsonl,
    run_llmff,
    write_classification_manifest,
)


QUEUE_MESSAGES = [
    {
        "id": "ticket-1001",
        "body": "Customer asks whether the failed invoice export will retry.",
    },
    {
        "id": "ticket-1002",
        "body": "Operations wants a summary before the morning handoff.",
    },
]


def write_queue_input(path: Path) -> None:
    lines = [
        f"{json.dumps(message, separators=(',', ':'))}\n"
        for message in QUEUE_MESSAGES
    ]
    path.write_text(
        "".join(lines),
        encoding="utf-8",
    )


def run_queue_worker(work_dir: Path) -> int:
    work_dir.mkdir(parents=True, exist_ok=True)
    batch_input = work_dir / "queue-items.jsonl"
    batch_output = work_dir / "queue-output"
    manifest = write_classification_manifest(work_dir, output_path="classification.json")
    write_queue_input(batch_input)

    env = os.environ.copy()
    env["LLMFF_MOCK_GOOD_RESPONSE"] = MOCK_CLASSIFICATION_RESPONSE

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
            str(manifest),
            "--batch-input",
            str(batch_input),
            "--batch-output-dir",
            str(batch_output),
            "--timeout-ms",
            "30000",
        ],
        cwd=work_dir,
        env=env,
    )

    report_path = batch_output / "batch-report.jsonl"
    rows = read_jsonl(report_path)
    failed = [row for row in rows if row.get("status") != "succeeded"]
    ack_failures = []

    print("workflow=queue-worker")
    print_inspect_summary(json.loads(inspect.stdout))
    print(f"batch_report={report_path}")
    print(f"queue_processed={len(rows)}")
    print(f"queue_failed={len(failed)}")
    for index, row in enumerate(rows):
        message_id = (
            QUEUE_MESSAGES[index]["id"] if index < len(QUEUE_MESSAGES) else "unknown"
        )
        item_id = f"{index:06}"
        output = batch_output / "items" / item_id / "classification.json"
        ack = row.get("status") == "succeeded" and output.exists()
        if not ack:
            ack_failures.append(message_id)
        print(f"queue_ack_{message_id}={str(ack).lower()}")

    if completed.returncode != 0:
        print_process_output(completed)
        return completed.returncode

    if len(rows) != len(QUEUE_MESSAGES) or failed or ack_failures:
        print("batch report does not match queue input", file=sys.stderr)
        return 1

    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--work-dir", type=Path)
    args = parser.parse_args()

    if args.work_dir:
        return run_queue_worker(args.work_dir)

    with tempfile.TemporaryDirectory(prefix="llmff-queue-worker-") as temp:
        return run_queue_worker(Path(temp))


if __name__ == "__main__":
    raise SystemExit(main())
