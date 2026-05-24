# llmff Specification

This file summarizes the current implementation, product goal, boundaries, and
open work for `llmff`.

## Current Implementation

`llmff` is a command-line and library runner for typed LLM inference pipelines.
It executes YAML manifests and compact inline graphs with explicit inputs,
ordered stages, backend adapters, local retrieval and reranking, JSON
validation, JSON repair, tool calls, caching, batch mode, checkpoint/resume,
JSONL traces, lifecycle events, and machine-readable inspection reports.

The current release line provides:

- deterministic mock backends for offline workflows and tests;
- OpenAI-compatible and Ollama backend adapters;
- manifest and inline graph execution;
- `inspect --format json` reproducibility reports;
- stable process exit codes and additive failure kinds;
- file-backed trace, event, checkpoint, batch, and output artifacts;
- plugin discovery and execution for stages, backends, samplers, and tool
  transports;
- release artifacts for Linux, macOS, and Windows through GitHub Releases.

## Product Goal

The goal is to make `llmff` a dependable FFmpeg-style runner for LLM inference
pipelines: boring process semantics, explicit inputs, reproducible manifests,
inspectable execution, local-first observability, restartable jobs, and stable
machine-readable outputs.

For agent systems, `llmff` should be a bounded execution tool. An agent or
supervisor decides what work should happen, selects or writes the manifest,
passes explicit inputs, runs `llmff`, and uses exit codes, failure kinds,
events, traces, checkpoints, and declared artifacts to decide the next action.

## Boundary With Agent Orchestration

`llmff` owns pipeline execution. It should answer: given this manifest or graph
and these inputs, what ran, what artifacts were produced, and what happened?

Agent orchestration systems own planning and context. OpenClaw, Hermes, or
similar hosts should own task decomposition, memory, tool selection policy,
multi-agent coordination, human approval, long-running control loops, global
configuration, and retry policy at the task level.

`llmff` is not a full agent framework. It should not own autonomous planning,
workspace indexing, persistent world models, global memory, or orchestration
loops. It should remain a clear execution substrate that those systems can
call, observe, and supervise.

## Supported Execution Model

The supported production shape is:

1. Inspect the manifest or inline graph before execution.
2. Run the pipeline with explicit inputs and file-backed artifacts.
3. Keep payload streams separate from metadata streams.
4. Treat the process exit code as the final authority.
5. Use `run_failed.failure_kind` and trace records for retry or escalation
   decisions.
6. Store trace, event, checkpoint, inspect, and output artifacts next to the
   supervising job record.

This keeps `llmff` useful from shells, CI jobs, daemons, and agent runtimes
without turning it into the scheduler or agent host.

## Production-Readiness Status

`llmff` is ready for early production-style internal workflows where callers
can supervise a subprocess, pin manifests in source control, store artifacts,
and tolerate a young provider/plugin ecosystem.

It is not yet as mature as FFmpeg. FFmpeg has decades of format coverage,
platform hardening, packaging availability, and ecosystem expectations.
`llmff` is still proving its contracts across real workflows, provider drift,
plugin adoption, and release-to-release compatibility.

The right production posture today is to use `llmff` for bounded jobs with
clear artifact ownership, local validation, and explicit supervision.

## Open Functionality Items

- Add more real-world examples that exercise current functionality without live
  credentials by default.
- Harden provider behavior through opt-in live smoke history, documented
  provider quirks, and support tiers.
- Improve production workflow examples for CI jobs, queue workers, scheduled
  jobs, long-running supervisors, and failure triage.
- Revisit manifest lockfile support only if it materially improves portability
  beyond `inspect --format json`.
- Grow plugin ecosystem confidence through reviewed external plugins and
  registry promotion policy.
- Expand local observability toward an optional OpenTelemetry bridge while
  preserving the file-based supervision contract.
- Prove schema, event, trace, and CLI compatibility over multiple releases.

## Distribution And Trust Items

- Keep GitHub Release assets and checksums as the default supported
  distribution lane.
- Keep Homebrew, Scoop, winget, and AUR metadata unpublished until maintainers
  decide each channel is support-ready.
- Keep apt repository publication parked until signed metadata, hosting, key
  rotation, retention, and recovery are designed.
- Keep Windows Authenticode signing, Apple Developer ID signing, and
  notarization parked until paid credentials and recovery procedures exist.
- Add SBOM/provenance artifacts only when release adoption justifies the
  additional support commitment.

## Real-World Example Roadmap

Examples should demonstrate useful workflows with the current implementation.
They must be offline-runnable by default, inspectable, documented, and tied to
validation gates.

The first catalog should cover:

- issue triage into structured JSON;
- meeting notes summarization and action extraction;
- local RAG answering with retrieval and reranking;
- batch classification with isolated item outputs.

Future examples should be added only when they show a distinct integration
pattern or operational behavior, not when they duplicate CLI reference
material.
