#!/usr/bin/env python3
"""Compose WisePick routing, llmff execution, and an Eventloom-style journal.

This is an external validation harness. It does not add WisePick or Eventloom
dependencies to llmff core.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any


DEFAULT_DECISION = {
    "decision_id": "dry_run_decision",
    "capability_id": "general_llm",
    "provider": "mock",
    "execution_type": "subprocess",
    "callable": True,
    "confidence": 1.0,
    "reason": "dry-run local validation",
}

MANIFEST_BY_CAPABILITY = {
    "general_llm": "json-repair",
    "json_repair": "json-repair",
    "classification": "json-repair",
    "default": "json-repair",
}


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def now_ms() -> int:
    return int(time.time() * 1000)


def resolve_command(command: str) -> str:
    if "/" not in command and "\\" not in command:
        return command
    return str(Path(command).resolve())


def append_eventloom_event(
    eventloom_bin: str,
    eventloom_log: Path,
    event_type: str,
    payload: dict[str, Any],
    thread_id: str,
) -> None:
    completed = subprocess.run(
        [
            resolve_command(eventloom_bin),
            "append",
            str(eventloom_log),
            event_type,
            "--actor",
            "wisepick-eventloom-flow",
            "--thread",
            thread_id,
            "--payload",
            json.dumps(payload, ensure_ascii=False, separators=(",", ":")),
        ],
        text=True,
        capture_output=True,
        check=False,
    )
    if completed.returncode != 0:
        message = completed.stderr.strip() or completed.stdout.strip() or "eventloom append failed"
        raise RuntimeError(message)


def write_event(
    journal: Path,
    event_type: str,
    payload: dict[str, Any],
    eventloom_log: Path | None = None,
    eventloom_bin: str = "eventloom",
    thread_id: str = "thread_wisepick_llmff_validation",
) -> None:
    event = {
        "type": event_type,
        "actor": "wisepick-eventloom-flow",
        "timestamp_ms": now_ms(),
        "payload": payload,
    }
    with journal.open("a", encoding="utf-8") as file:
        file.write(json.dumps(event, ensure_ascii=False, separators=(",", ":")) + "\n")
    if eventloom_log:
        append_eventloom_event(eventloom_bin, eventloom_log, event_type, payload, thread_id)


def post_json(url: str, payload: dict[str, Any], timeout: float) -> dict[str, Any]:
    request = urllib.request.Request(
        url,
        data=json.dumps(payload, ensure_ascii=False).encode("utf-8"),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            body = response.read().decode("utf-8", errors="replace")
    except (urllib.error.HTTPError, urllib.error.URLError, TimeoutError, OSError) as exc:
        raise RuntimeError(f"POST {url} failed: {exc}") from exc
    try:
        data = json.loads(body)
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"POST {url} returned non-JSON response") from exc
    if not isinstance(data, dict):
        raise RuntimeError(f"POST {url} returned JSON {type(data).__name__}, expected object")
    return data


def decide(api_url: str, intent: str, timeout: float) -> dict[str, Any]:
    return post_json(f"{api_url.rstrip('/')}/v1/decide", {"task": intent}, timeout)


def send_feedback(api_url: str, payload: dict[str, Any], timeout: float) -> dict[str, Any]:
    return post_json(f"{api_url.rstrip('/')}/v1/feedback", payload, timeout)


def copy_json_repair_pipeline(out_dir: Path) -> Path:
    source_dir = repo_root() / "examples"
    work_dir = out_dir / "pipeline"
    work_dir.mkdir(parents=True, exist_ok=True)
    for name in [
        "json-repair.yaml",
        "question.txt",
        "prompt.tmpl",
        "policy.md",
        "answer.schema.json",
    ]:
        shutil.copy2(source_dir / name, work_dir / name)
    return work_dir / "json-repair.yaml"


def choose_manifest(decision: dict[str, Any], out_dir: Path) -> Path:
    capability = str(decision.get("capability_id") or "default")
    manifest_name = MANIFEST_BY_CAPABILITY.get(capability, MANIFEST_BY_CAPABILITY["default"])
    if manifest_name != "json-repair":
        raise RuntimeError(f"unsupported manifest mapping {manifest_name!r}")
    return copy_json_repair_pipeline(out_dir)


def run_llmff(llmff_bin: str, manifest: Path, out_dir: Path, timeout_ms: int) -> tuple[int, int]:
    events_path = out_dir / "llmff-events.jsonl"
    trace_path = out_dir / "llmff-trace.jsonl"
    checkpoint_path = out_dir / "llmff-checkpoint.json"
    env = os.environ.copy()
    env.setdefault("LLMFF_MOCK_BAD_RESPONSE", '{"wrong":true}')
    env.setdefault("LLMFF_MOCK_GOOD_RESPONSE", '{"answer":"ok"}')

    started = time.monotonic()
    completed = subprocess.run(
        [
            resolve_command(llmff_bin),
            "run",
            str(manifest),
            "--events",
            str(events_path),
            "--trace",
            str(trace_path),
            "--checkpoint",
            str(checkpoint_path),
            "--timeout-ms",
            str(timeout_ms),
        ],
        cwd=manifest.parent,
        env=env,
        text=True,
        capture_output=True,
        check=False,
        timeout=max(1, int(timeout_ms / 1000) + 5),
    )
    elapsed_ms = int((time.monotonic() - started) * 1000)
    if completed.stdout:
        (out_dir / "llmff-stdout.txt").write_text(completed.stdout, encoding="utf-8")
    if completed.stderr:
        (out_dir / "llmff-stderr.txt").write_text(completed.stderr, encoding="utf-8")
    return completed.returncode, elapsed_ms


def build_feedback(decision: dict[str, Any], success: bool, latency_ms: int) -> dict[str, Any]:
    return {
        "decision_id": str(decision.get("decision_id") or ""),
        "success": success,
        "latency_ms": latency_ms,
        "token_cost": {"input": 0, "output": 0},
        "result_quality": 1.0 if success else 0.0,
        "user_note": "llmff validation harness",
        "runtime_name": "llmff-wisepick-eventloom-flow",
    }


def run(args: argparse.Namespace) -> int:
    out_dir = args.out_dir.resolve()
    out_dir.mkdir(parents=True, exist_ok=True)
    journal = out_dir / "eventloom-compatible.jsonl"
    eventloom_log = args.eventloom_log.resolve() if args.eventloom_log else None
    if eventloom_log:
        eventloom_log.parent.mkdir(parents=True, exist_ok=True)
    if journal.exists():
        journal.unlink()

    def record(event_type: str, payload: dict[str, Any]) -> None:
        write_event(
            journal,
            event_type,
            payload,
            eventloom_log=eventloom_log,
            eventloom_bin=args.eventloom_bin,
            thread_id=args.eventloom_thread,
        )

    api_url = args.api_url or os.environ.get("WISEPICK_API_URL", "").strip()
    record(
        "routing.decide.requested",
        {"intent": args.intent, "dry_run": args.dry_run, "mock_wisepick": args.mock_wisepick},
    )

    if args.dry_run or args.mock_wisepick:
        decision = dict(DEFAULT_DECISION)
    else:
        if not api_url:
            print("WISEPICK_API_URL or --api-url is required unless --dry-run is set", file=sys.stderr)
            return 2
        decision = decide(api_url, args.intent, args.http_timeout)

    record("routing.decided", {"decision": decision})
    manifest = choose_manifest(decision, out_dir)

    if args.dry_run:
        record(
            "llmff.execution.planned",
            {
                "manifest": str(manifest),
                "llmff_bin": args.llmff_bin,
                "timeout_ms": args.timeout_ms,
            },
        )
        feedback = build_feedback(decision, True, 0)
        record("routing.feedback.planned", {"feedback": feedback})
        print(f"dry_run=true journal={journal}")
        return 0

    record("llmff.execution.started", {"manifest": str(manifest)})
    returncode, latency_ms = run_llmff(args.llmff_bin, manifest, out_dir, args.timeout_ms)
    success = returncode == 0
    record(
        "llmff.execution.finished",
        {"returncode": returncode, "success": success, "latency_ms": latency_ms},
    )

    feedback = build_feedback(decision, success, latency_ms)
    if args.mock_wisepick:
        record("routing.feedback.planned", {"feedback": feedback})
        print(f"mock_wisepick=true success={str(success).lower()} journal={journal}")
        return 0 if success else returncode

    record("routing.feedback.requested", {"feedback": feedback})
    response = send_feedback(api_url, feedback, args.http_timeout)
    record("routing.feedback.completed", {"response": response})

    print(f"success={str(success).lower()} journal={journal}")
    return 0 if success else returncode


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Validate WisePick -> llmff -> Eventloom-compatible JSONL -> WisePick feedback."
    )
    parser.add_argument("--intent", required=True, help="Natural-language routing intent.")
    parser.add_argument("--out-dir", type=Path, default=Path(".llmff/wisepick-eventloom-flow"))
    parser.add_argument("--api-url", help="WisePick API base URL. Defaults to WISEPICK_API_URL.")
    parser.add_argument("--llmff-bin", default=os.environ.get("LLMFF_BIN", "llmff"))
    parser.add_argument("--eventloom-bin", default=os.environ.get("EVENTLOOM_BIN", "eventloom"))
    parser.add_argument("--eventloom-log", type=Path, help="Optional sealed Eventloom log path.")
    parser.add_argument(
        "--eventloom-thread",
        default="thread_wisepick_llmff_validation",
        help="Eventloom thread id used when --eventloom-log is set.",
    )
    parser.add_argument("--timeout-ms", type=int, default=30000)
    parser.add_argument("--http-timeout", type=float, default=10.0)
    parser.add_argument("--dry-run", action="store_true", help="Do not call WisePick or llmff.")
    parser.add_argument(
        "--mock-wisepick",
        action="store_true",
        help="Use a synthetic WisePick decision, run llmff, and write planned feedback without HTTP.",
    )
    return run(parser.parse_args())


if __name__ == "__main__":
    raise SystemExit(main())
