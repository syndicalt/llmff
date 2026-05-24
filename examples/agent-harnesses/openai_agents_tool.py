#!/usr/bin/env python3
"""OpenAI Agents SDK adapter for running llmff as a bounded subprocess."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Any


SAFE_ID = re.compile(r"[^A-Za-z0-9_.-]+")


@dataclass(frozen=True)
class LlmffToolConfig:
    """Configuration for an OpenAI Agents SDK tool backed by llmff."""

    manifest: Path
    run_root: Path
    input_placeholder: str = "{{LLMFF_INPUT_PATH}}"
    llmff_bin: str | None = None
    process_timeout_seconds: float = 65.0
    llmff_timeout_ms: int = 60_000


@dataclass(frozen=True)
class LlmffRunArtifacts:
    """Artifacts produced by one llmff subprocess invocation."""

    run_dir: Path
    result_path: Path
    events_path: Path
    trace_path: Path
    returncode: int


class LlmffRunError(RuntimeError):
    """Raised when llmff exits non-zero or does not produce result.json."""

    def __init__(
        self,
        message: str,
        *,
        artifacts: LlmffRunArtifacts,
        failure_kind: str | None = None,
        failure_message: str | None = None,
        stderr: str = "",
    ) -> None:
        super().__init__(message)
        self.artifacts = artifacts
        self.failure_kind = failure_kind
        self.failure_message = failure_message
        self.stderr = stderr


def _resolve_llmff_bin(configured: str | None) -> str:
    command = configured or os.environ.get("LLMFF_BIN") or "llmff"
    if "/" not in command and "\\" not in command:
        return command
    return str(Path(command).resolve())


def _safe_run_id(value: str) -> str:
    normalized = SAFE_ID.sub("-", value.strip())[:80].strip(".-")
    if normalized:
        return normalized
    digest = hashlib.sha256(value.encode("utf-8")).hexdigest()[:16]
    return f"llmff-{digest}"


def _read_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        value = json.load(handle)
    if not isinstance(value, dict):
        raise ValueError(f"{path} must contain a JSON object")
    return value


def _read_failure_event(path: Path) -> tuple[str | None, str | None]:
    if not path.exists():
        return None, None

    failure_kind: str | None = None
    failure_message: str | None = None
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(event, dict) and event.get("event") == "run_failed":
            kind = event.get("failure_kind")
            message = event.get("failure_message")
            failure_kind = kind if isinstance(kind, str) else None
            failure_message = message if isinstance(message, str) else None
    return failure_kind, failure_message


def _build_command(
    config: LlmffToolConfig, run_dir: Path, manifest_path: Path | None = None
) -> list[str]:
    return [
        _resolve_llmff_bin(config.llmff_bin),
        "run",
        "--run-dir",
        str(run_dir),
        str(manifest_path or config.manifest),
        "--timeout-ms",
        str(config.llmff_timeout_ms),
    ]


def _materialize_manifest(config: LlmffToolConfig, run_dir: Path, task_input: str) -> Path:
    template = config.manifest.read_text(encoding="utf-8")
    if config.input_placeholder not in template:
        raise ValueError(
            f"{config.manifest} must contain {config.input_placeholder!r} so each "
            "agent call gets an isolated run-scoped input file"
        )
    input_path = run_dir / "input.txt"
    input_path.write_text(task_input, encoding="utf-8")
    manifest_path = run_dir / "manifest.yaml"
    manifest_path.write_text(
        template.replace(config.input_placeholder, str(input_path)),
        encoding="utf-8",
    )
    return manifest_path


def run_llmff_pipeline(
    *,
    config: LlmffToolConfig,
    task_input: str,
    run_id: str | None = None,
) -> dict[str, Any]:
    """Run llmff and return the JSON object written to result.json."""

    config.run_root.mkdir(parents=True, exist_ok=True)
    run_dir = config.run_root / _safe_run_id(run_id or f"llmff-{uuid.uuid4().hex}")
    run_dir.mkdir(parents=True, exist_ok=False)
    manifest_path = _materialize_manifest(config, run_dir, task_input)

    command = _build_command(config, run_dir, manifest_path)
    try:
        completed = subprocess.run(
            command,
            text=True,
            capture_output=True,
            check=False,
            timeout=config.process_timeout_seconds,
        )
    except subprocess.TimeoutExpired as exc:
        artifacts = LlmffRunArtifacts(
            run_dir=run_dir,
            result_path=run_dir / "result.json",
            events_path=run_dir / "events.jsonl",
            trace_path=run_dir / "trace.jsonl",
            returncode=124,
        )
        raise LlmffRunError(
            f"llmff timed out after {config.process_timeout_seconds} seconds",
            artifacts=artifacts,
            stderr=(exc.stderr or "") if isinstance(exc.stderr, str) else "",
        ) from exc

    artifacts = LlmffRunArtifacts(
        run_dir=run_dir,
        result_path=run_dir / "result.json",
        events_path=run_dir / "events.jsonl",
        trace_path=run_dir / "trace.jsonl",
        returncode=completed.returncode,
    )

    if completed.returncode != 0:
        failure_kind, failure_message = _read_failure_event(artifacts.events_path)
        raise LlmffRunError(
            f"llmff exited with status {completed.returncode}",
            artifacts=artifacts,
            failure_kind=failure_kind,
            failure_message=failure_message,
            stderr=completed.stderr,
        )

    if not artifacts.result_path.exists():
        raise LlmffRunError(
            "llmff completed without writing result.json",
            artifacts=artifacts,
            stderr=completed.stderr,
        )

    return _read_json(artifacts.result_path)


def build_openai_agents_tool(config: LlmffToolConfig) -> Any:
    """Return an OpenAI Agents SDK tool, importing the SDK only on demand."""

    try:
        from agents import function_tool
    except ImportError as exc:
        raise RuntimeError(
            "OpenAI Agents SDK is required to register this tool. "
            "Install it with: python3 -m pip install openai-agents"
        ) from exc

    @function_tool
    def run_llmff_manifest(task_input: str, run_id: str | None = None) -> dict[str, Any]:
        """Run the configured llmff manifest and return result.json."""

        return run_llmff_pipeline(
            config=config,
            task_input=task_input,
            run_id=run_id,
        )

    return run_llmff_manifest


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="OpenAI Agents SDK llmff tool adapter example."
    )
    parser.add_argument("--manifest", type=Path, default=Path("pipeline.yaml"))
    parser.add_argument("--run-root", type=Path, default=Path(".llmff/openai-agents"))
    parser.add_argument("--input-placeholder", default="{{LLMFF_INPUT_PATH}}")
    parser.add_argument("--task-input", default="example task")
    parser.add_argument("--run-id")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument(
        "--register-tool",
        action="store_true",
        help="Import the OpenAI Agents SDK and build the tool object.",
    )
    return parser.parse_args()


def main() -> int:
    args = _parse_args()
    config = LlmffToolConfig(
        manifest=args.manifest,
        run_root=args.run_root,
        input_placeholder=args.input_placeholder,
    )

    if args.register_tool:
        try:
            build_openai_agents_tool(config)
        except RuntimeError as exc:
            print(str(exc), file=sys.stderr)
            return 2
        print("registered=openai_agents_tool")
        return 0

    run_dir = args.run_root / _safe_run_id(args.run_id or f"llmff-{uuid.uuid4().hex}")
    if args.dry_run:
        print(" ".join(_build_command(config, run_dir)))
        return 0

    result = run_llmff_pipeline(
        config=config,
        task_input=args.task_input,
        run_id=args.run_id,
    )
    print(json.dumps(result, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
