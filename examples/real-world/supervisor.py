#!/usr/bin/env python3
"""Supervise the issue-triage real-world workflow as a bounded subprocess."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


MOCK_TRIAGE_RESPONSE = json.dumps(
    {
        "category": "operations",
        "priority": "high",
        "summary": "Nightly invoice export times out before finance close.",
        "recommended_action": (
            "Escalate to the job owner, collect trace artifacts, "
            "and provide a same-day workaround."
        ),
    },
    separators=(",", ":"),
)


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def llmff_command() -> str:
    configured = os.environ.get("LLMFF_BIN")
    if configured:
        return configured
    return str(Path("target/debug/llmff"))


def run_llmff(
    args: list[str], env: dict[str, str]
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [llmff_command(), *args],
        cwd=repo_root(),
        env=env,
        text=True,
        capture_output=True,
        check=False,
        timeout=60,
    )


def write_process_output(completed: subprocess.CompletedProcess[str]) -> None:
    if completed.stdout:
        print(completed.stdout, end="")
    if completed.stderr:
        print(completed.stderr, file=sys.stderr, end="")


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        return []

    events: list[dict[str, Any]] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict):
            events.append(value)
    return events


def print_failure_summary(events_path: Path) -> None:
    failures = [
        event
        for event in load_jsonl(events_path)
        if event.get("event") == "run_failed"
        and (event.get("failure_kind") or event.get("failure_message"))
    ]
    if not failures:
        return

    failure = failures[-1]
    print(
        "failure_kind="
        f"{failure.get('failure_kind', 'unknown')} "
        f"failure_message={failure.get('failure_message', '')}",
        file=sys.stderr,
    )


def output_artifact(report: dict[str, Any]) -> tuple[str, Path]:
    outputs = report.get("outputs")
    if not isinstance(outputs, dict) or "final" not in outputs:
        raise ValueError("inspect report does not declare a final output artifact")

    final_output = outputs["final"]
    if not isinstance(final_output, dict) or not isinstance(
        final_output.get("path"), str
    ):
        raise ValueError("inspect report final output is missing a path")

    source = report.get("manifest", {}).get("source", {})
    source_cwd = source.get("cwd") if isinstance(source, dict) else None
    output_relative = Path(final_output["path"])
    display_path = output_relative
    absolute_path = repo_root() / output_relative

    if isinstance(source_cwd, str) and source_cwd:
        display_path = Path(source_cwd) / output_relative
        absolute_path = repo_root() / source_cwd / output_relative

    return display_path.as_posix(), absolute_path


def run_supervised(run_dir: Path) -> int:
    run_dir.mkdir(parents=True, exist_ok=True)
    manifest = Path("examples/real-world/issue-triage.yaml")
    inspect_path = run_dir / "inspect.json"
    trace_path = run_dir / "trace.jsonl"
    events_path = run_dir / "events.jsonl"

    env = os.environ.copy()
    env["LLMFF_MOCK_GOOD_RESPONSE"] = MOCK_TRIAGE_RESPONSE

    inspect = run_llmff(
        ["inspect", str(manifest), "--format", "json"],
        env,
    )
    inspect_path.write_text(inspect.stdout, encoding="utf-8")
    if inspect.returncode != 0:
        write_process_output(inspect)
        return inspect.returncode

    report = json.loads(inspect.stdout)
    display_output, artifact_path = output_artifact(report)

    completed = run_llmff(
        [
            "run",
            str(manifest),
            "--trace",
            str(trace_path),
            "--events",
            str(events_path),
        ],
        env,
    )

    print(f"inspect={inspect_path}")
    print(f"trace={trace_path}")
    print(f"events={events_path}")
    print(f"run_status={'ok' if completed.returncode == 0 else 'failed'}")

    if completed.returncode != 0:
        write_process_output(completed)
        print_failure_summary(events_path)
        return completed.returncode

    if not artifact_path.exists():
        print(f"declared output artifact missing: {display_output}", file=sys.stderr)
        return 1

    print(f"output={display_output}")
    print("output_exists=true")
    return completed.returncode


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--run-dir",
        type=Path,
        help="Directory for inspect.json, trace.jsonl, and events.jsonl.",
    )
    args = parser.parse_args()

    if args.run_dir:
        return run_supervised(args.run_dir)

    run_dir = Path(tempfile.mkdtemp(prefix="llmff-real-world-"))
    return run_supervised(run_dir)


if __name__ == "__main__":
    raise SystemExit(main())
