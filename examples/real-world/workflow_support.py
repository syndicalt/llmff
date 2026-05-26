"""Helpers shared by the real-world job-runner examples."""

from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path
from typing import Any


MOCK_CLASSIFICATION_RESPONSE = json.dumps(
    {
        "label": "support",
        "confidence": 0.91,
        "rationale": "The item asks for operational guidance.",
    },
    separators=(",", ":"),
)
MOCK_MEETING_RESPONSE = json.dumps(
    {
        "summary": (
            "The team kept llmff focused on bounded execution and deferred "
            "package-manager publication."
        ),
    "decisions": [
        "llmff remains an execution substrate, not an agent framework.",
    ],
        "actions": [
            {"owner": "Dana", "task": "Draft production examples."},
            {"owner": "Ravi", "task": "Review provider smoke expectations."},
        ],
    },
    separators=(",", ":"),
)
MOCK_TRIAGE_RESPONSE = json.dumps(
    {
        "category": "operations",
        "priority": "high",
        "summary": "Nightly invoice export times out before finance close.",
        "recommended_action": (
            "Escalate to the job owner, collect trace artifacts, and provide "
            "a same-day workaround."
        ),
    },
    separators=(",", ":"),
)


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def real_world_path(*parts: str) -> Path:
    return repo_root() / "examples" / "real-world" / Path(*parts)


def llmff_command() -> str:
    configured = os.environ.get("LLMFF_BIN")
    if configured:
        return configured
    return str(repo_root() / "target" / "debug" / "llmff")


def run_llmff(
    args: list[str],
    *,
    cwd: Path,
    env: dict[str, str],
    timeout: int = 60,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [llmff_command(), *args],
        cwd=cwd,
        env=env,
        text=True,
        capture_output=True,
        check=False,
        timeout=timeout,
    )


def shell_safe_path(path: Path | str) -> str:
    return json.dumps(str(path))


def write_issue_manifest(work_dir: Path, *, output_path: str) -> Path:
    manifest = work_dir / "issue-triage.yaml"
    input_path = shell_safe_path(real_world_path("inputs", "support-issue.txt"))
    schema_path = shell_safe_path(real_world_path("schemas", "issue-triage.schema.json"))
    manifest.write_text(
        f"""version: 1
inputs:
  issue:
    path: {input_path}
graph:
  - id: load_issue
    op: load
    input: issue
  - id: triage
    op: infer
    from: load_issue
    model: mock:good
    temperature: 0.0
    max_tokens: 256
    response_format: json
  - id: validate_triage
    op: validate_json
    from: triage
    schema_path: {schema_path}
outputs:
  final:
    from: validate_triage
    path: {shell_safe_path(output_path)}
""",
        encoding="utf-8",
    )
    return manifest


def write_meeting_manifest(work_dir: Path, *, output_path: str) -> Path:
    manifest = work_dir / "meeting-notes.yaml"
    input_path = shell_safe_path(real_world_path("inputs", "meeting-notes.txt"))
    schema_path = shell_safe_path(real_world_path("schemas", "meeting-notes.schema.json"))
    manifest.write_text(
        f"""version: 1
inputs:
  notes:
    path: {input_path}
graph:
  - id: load_notes
    op: load
    input: notes
  - id: extract_decisions
    op: infer
    from: load_notes
    model: mock:good
    temperature: 0.0
    max_tokens: 512
    response_format: json
  - id: validate_notes
    op: validate_json
    from: extract_decisions
    schema_path: {schema_path}
outputs:
  final:
    from: validate_notes
    path: {shell_safe_path(output_path)}
""",
        encoding="utf-8",
    )
    return manifest


def write_classification_manifest(work_dir: Path, *, output_path: str) -> Path:
    manifest = work_dir / "classification.yaml"
    schema_path = shell_safe_path(
        real_world_path("schemas", "classification.schema.json")
    )
    manifest.write_text(
        f"""version: 1
inputs:
  item:
    path: queue-items.jsonl
graph:
  - id: load_item
    op: load
    input: item
    input_format: json
  - id: classify
    op: infer
    from: load_item
    model: mock:good
    temperature: 0.0
    max_tokens: 256
    response_format: json
  - id: validate_classification
    op: validate_json
    from: classify
    schema_path: {schema_path}
outputs:
  final:
    from: validate_classification
    path: {shell_safe_path(output_path)}
""",
        encoding="utf-8",
    )
    return manifest


def read_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        return []
    rows: list[dict[str, Any]] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.strip():
            rows.append(json.loads(line))
    return rows


def print_process_output(completed: subprocess.CompletedProcess[str]) -> None:
    if completed.stdout:
        print(completed.stdout, end="")
    if completed.stderr:
        import sys

        print(completed.stderr, file=sys.stderr, end="")


def print_inspect_summary(inspect: dict[str, Any]) -> None:
    print(f"inspect_format_version={inspect['format_version']}")
    print(f"manifest_hash={inspect['manifest']['hash']}")
