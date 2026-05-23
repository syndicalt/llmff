#!/usr/bin/env python3
"""Run an offline llmff batch job as a supervised agent subprocess."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path


def resolve_command(command: str) -> str:
    if "/" not in command and "\\" not in command:
        return command
    return str(Path(command).resolve())


def write_fixture(work_dir: Path) -> tuple[Path, Path, Path]:
    manifest = work_dir / "batch-pipeline.yaml"
    placeholder = work_dir / "placeholder.txt"
    batch_input = work_dir / "items.txt"
    batch_output = work_dir / "batch-output"

    placeholder.write_text("inspect placeholder\n", encoding="utf-8")
    batch_input.write_text("first item\nsecond item\n", encoding="utf-8")
    manifest.write_text(
        """version: 1
inputs:
  prompt:
    path: placeholder.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
outputs:
  final:
    from: load_prompt
    path: answer.txt
""",
        encoding="utf-8",
    )
    return manifest, batch_input, batch_output


def read_report(report_path: Path) -> list[dict[str, object]]:
    rows = []
    if not report_path.exists():
        return rows
    for line in report_path.read_text(encoding="utf-8").splitlines():
        if line.strip():
            rows.append(json.loads(line))
    return rows


def run_pipeline(work_dir: Path) -> int:
    manifest, batch_input, batch_output = write_fixture(work_dir)
    llmff = resolve_command(os.environ.get("LLMFF_BIN", "llmff"))

    inspect = subprocess.run(
        [llmff, "inspect", str(manifest), "--format", "json"],
        cwd=work_dir,
        text=True,
        capture_output=True,
        check=False,
    )
    if inspect.returncode != 0:
        if inspect.stderr:
            print(inspect.stderr, file=sys.stderr, end="")
        return inspect.returncode

    report = json.loads(inspect.stdout)
    print(f"inspect_format_version={report['format_version']}")
    print(f"manifest_hash={report['manifest']['hash']}")
    print(
        "stdout_manifest_outputs="
        f"{str(report['execution']['stdout']['manifest_outputs']).lower()}"
    )

    completed = subprocess.run(
        [
            llmff,
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
        text=True,
        capture_output=True,
        check=False,
    )

    report_path = batch_output / "batch-report.jsonl"
    rows = read_report(report_path)
    failed_count = sum(1 for row in rows if row.get("status") != "succeeded")
    status = "ok" if completed.returncode == 0 else "failed"

    print(f"run_status={status}")
    print(f"batch_report={report_path}")
    print(f"item_count={len(rows)}")
    print(f"failed_count={failed_count}")
    for index in range(len(rows)):
        item_id = f"{index:06}"
        output = batch_output / "items" / item_id / "answer.txt"
        print(f"item_{item_id}_output={output}")
        print(f"item_{item_id}_output_exists={str(output.exists()).lower()}")

    if completed.stderr:
        print(completed.stderr, file=sys.stderr, end="")

    return completed.returncode


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--work-dir",
        type=Path,
        help="Directory for generated manifest, batch inputs, and outputs.",
    )
    args = parser.parse_args()

    if args.work_dir:
        args.work_dir.mkdir(parents=True, exist_ok=True)
        return run_pipeline(args.work_dir)

    with tempfile.TemporaryDirectory(prefix="llmff-agent-batch-") as temp:
        return run_pipeline(Path(temp))


if __name__ == "__main__":
    raise SystemExit(main())
