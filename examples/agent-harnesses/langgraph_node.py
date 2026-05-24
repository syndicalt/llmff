#!/usr/bin/env python3
"""LangGraph node adapter for running llmff as a bounded subprocess."""

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
from typing import Any, Mapping


SAFE_ID = re.compile(r"[^A-Za-z0-9_.-]+")


@dataclass(frozen=True)
class LlmffNodeConfig:
    """Configuration for a LangGraph-compatible llmff node."""

    manifest: Path
    run_root: Path
    input_placeholder: str = "{{LLMFF_INPUT_PATH}}"
    input_key: str = "task"
    output_key: str = "llmff_result"
    llmff_bin: str | None = None
    process_timeout_seconds: float = 65.0
    llmff_timeout_ms: int = 60_000


@dataclass(frozen=True)
class LlmffRunMetadata:
    """Operational metadata returned alongside the node payload."""

    run_dir: str
    result_path: str
    events_path: str
    trace_path: str
    returncode: int


class LlmffNodeError(RuntimeError):
    """Raised when the node cannot complete the llmff run."""

    def __init__(
        self,
        message: str,
        *,
        metadata: LlmffRunMetadata,
        failure_kind: str | None = None,
        failure_message: str | None = None,
        stderr: str = "",
    ) -> None:
        super().__init__(message)
        self.metadata = metadata
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


def _metadata(run_dir: Path, returncode: int) -> LlmffRunMetadata:
    return LlmffRunMetadata(
        run_dir=str(run_dir),
        result_path=str(run_dir / "result.json"),
        events_path=str(run_dir / "events.jsonl"),
        trace_path=str(run_dir / "trace.jsonl"),
        returncode=returncode,
    )


def _build_command(
    config: LlmffNodeConfig, run_dir: Path, manifest_path: Path | None = None
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


def _materialize_manifest(
    config: LlmffNodeConfig, run_dir: Path, serialized_input: str
) -> Path:
    template = config.manifest.read_text(encoding="utf-8")
    if config.input_placeholder not in template:
        raise ValueError(
            f"{config.manifest} must contain {config.input_placeholder!r} so each "
            "LangGraph call gets an isolated run-scoped input file"
        )
    input_path = run_dir / "input.json"
    input_path.write_text(serialized_input, encoding="utf-8")
    manifest_path = run_dir / "manifest.yaml"
    manifest_path.write_text(
        template.replace(config.input_placeholder, str(input_path)),
        encoding="utf-8",
    )
    return manifest_path


def run_llmff_for_state(
    *,
    config: LlmffNodeConfig,
    state: Mapping[str, Any],
) -> tuple[dict[str, Any], LlmffRunMetadata]:
    """Run llmff for one LangGraph state object."""

    if config.input_key not in state:
        raise KeyError(f"state is missing required key: {config.input_key}")

    task_input = state[config.input_key]
    serialized_input = json.dumps(task_input, sort_keys=True, default=str)
    run_id_source = str(state.get("run_id") or f"llmff-{uuid.uuid4().hex}")
    config.run_root.mkdir(parents=True, exist_ok=True)
    run_dir = config.run_root / _safe_run_id(run_id_source)
    run_dir.mkdir(parents=True, exist_ok=False)
    manifest_path = _materialize_manifest(config, run_dir, serialized_input)

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
        metadata = _metadata(run_dir, 124)
        raise LlmffNodeError(
            f"llmff timed out after {config.process_timeout_seconds} seconds",
            metadata=metadata,
            stderr=(exc.stderr or "") if isinstance(exc.stderr, str) else "",
        ) from exc

    metadata = _metadata(run_dir, completed.returncode)
    events_path = Path(metadata.events_path)
    result_path = Path(metadata.result_path)

    if completed.returncode != 0:
        failure_kind, failure_message = _read_failure_event(events_path)
        raise LlmffNodeError(
            f"llmff exited with status {completed.returncode}",
            metadata=metadata,
            failure_kind=failure_kind,
            failure_message=failure_message,
            stderr=completed.stderr,
        )

    if not result_path.exists():
        raise LlmffNodeError(
            "llmff completed without writing result.json",
            metadata=metadata,
            stderr=completed.stderr,
        )

    return _read_json(result_path), metadata


class LlmffRunNode:
    """Callable node that returns a LangGraph state update."""

    def __init__(self, config: LlmffNodeConfig) -> None:
        self.config = config

    def __call__(self, state: Mapping[str, Any]) -> dict[str, Any]:
        result, metadata = run_llmff_for_state(config=self.config, state=state)
        return {
            self.config.output_key: result,
            "llmff_run": {
                "run_dir": metadata.run_dir,
                "result_path": metadata.result_path,
                "events_path": metadata.events_path,
                "trace_path": metadata.trace_path,
                "returncode": metadata.returncode,
            },
        }


def build_langgraph_node(config: LlmffNodeConfig) -> LlmffRunNode:
    """Return a LangGraph-compatible callable without importing LangGraph."""

    return LlmffRunNode(config)


def assert_langgraph_available() -> None:
    """Validate optional LangGraph availability only for framework execution."""

    try:
        import langgraph  # noqa: F401
    except ImportError as exc:
        raise RuntimeError(
            "LangGraph is required to execute a graph with this node. "
            "Install it with: python3 -m pip install langgraph"
        ) from exc


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="LangGraph llmff node adapter example.")
    parser.add_argument("--manifest", type=Path, default=Path("pipeline.yaml"))
    parser.add_argument("--run-root", type=Path, default=Path(".llmff/langgraph"))
    parser.add_argument("--input-placeholder", default="{{LLMFF_INPUT_PATH}}")
    parser.add_argument("--input-key", default="task")
    parser.add_argument("--output-key", default="llmff_result")
    parser.add_argument("--task-input", default="example task")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument(
        "--check-langgraph",
        action="store_true",
        help="Import LangGraph and fail with installation guidance if unavailable.",
    )
    return parser.parse_args()


def main() -> int:
    args = _parse_args()
    if args.check_langgraph:
        try:
            assert_langgraph_available()
        except RuntimeError as exc:
            print(str(exc), file=sys.stderr)
            return 2
        print("langgraph_available=true")
        return 0

    config = LlmffNodeConfig(
        manifest=args.manifest,
        run_root=args.run_root,
        input_placeholder=args.input_placeholder,
        input_key=args.input_key,
        output_key=args.output_key,
    )
    state = {args.input_key: args.task_input}
    run_dir = args.run_root / _safe_run_id(f"llmff-{uuid.uuid4().hex}")

    if args.dry_run:
        print(" ".join(_build_command(config, run_dir)))
        return 0

    update = build_langgraph_node(config)(state)
    print(json.dumps(update, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
