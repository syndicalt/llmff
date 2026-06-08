# llmff Specification

This is the canonical product boundary and roadmap document for `llmff`.

`llmff` is a bounded FFmpeg-style execution runner for LLM inference
pipelines. It executes declared manifests and graphs; it does not plan work,
own memory, coordinate agents, or act as an agent framework.

First-reader decision guidance lives in
[`docs/when-to-use-llmff.md`](docs/when-to-use-llmff.md). Workflow recipes live
in [`docs/cookbook.md`](docs/cookbook.md), and the pre-1.0 compatibility
checklist lives in
[`docs/migration/pre-1.0-to-1.0.md`](docs/migration/pre-1.0-to-1.0.md).

## Current Implementation

`llmff` is currently a command-line and library runner for typed LLM inference
pipelines. It executes YAML manifests and compact inline graphs with explicit
inputs, ordered stages, backend adapters, local retrieval and reranking, JSON
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

The goal is to make `llmff` a dependable execution substrate for LLM inference
pipelines: explicit inputs, reproducible manifests, boring process semantics,
inspectable execution, restartable jobs, local-first observability, and stable
machine-readable outputs.

For shells, CI jobs, daemons, and agent runtimes, `llmff` should behave like a
supervisable subprocess. A caller chooses or writes the manifest, passes
explicit inputs, runs `llmff`, and makes follow-up decisions from exit codes,
failure kinds, events, traces, checkpoints, and declared artifacts.
In agent systems, that makes `llmff` a bounded execution tool rather than the
agent host.

## Supported Execution Contract

The supported production contract is:

1. Inspect the manifest or inline graph before execution.
2. Run the pipeline with explicit inputs and file-backed artifacts.
3. Keep payload streams separate from metadata streams.
4. Treat the process exit code as the final authority for run success.
5. Use additive failure kinds and trace records for retry or escalation
   decisions.
6. Store trace, event, checkpoint, inspect, and output artifacts next to the
   supervising job record.

The contract is intentionally process-oriented. `llmff` owns what ran, what
failed, what artifacts were produced, and what machine-readable evidence was
emitted. The caller owns why the job exists and what should happen next.

## Production-Readiness Criteria

`llmff` is production-ready for a workflow when all of the following are true:

- the manifest or graph is pinned, inspected, and reviewed with the calling
  system;
- inputs, outputs, checkpoints, traces, events, and inspect reports are stored
  under caller-owned artifact retention;
- the caller supervises the process, handles non-zero exit codes, and maps
  failure kinds to retry, escalation, or terminal failure;
- backend credentials and provider-specific limits are managed outside
  `llmff`;
- compatibility-sensitive consumers depend only on documented CLI, schema,
  event, trace, artifact, and exit-code contracts;
- live provider behavior is covered by the caller's smoke tests where provider
  drift would affect production outcomes.

Today, the expected use is bounded production-style internal workflows where a
caller can supervise subprocess execution, pin manifests in source control,
store artifacts, and tolerate a young provider and plugin ecosystem.

## Explicitly Not Ready

`llmff` is not yet as mature as FFmpeg. It has not proven decades of format
coverage, platform hardening, distribution availability, ecosystem norms, or
release-to-release compatibility at that scale.

The following areas are not ready to treat as fully settled production
infrastructure:

- broad provider support tiers and documented provider quirk handling;
- long-lived compatibility proof across many releases;
- mature external plugin review, registry promotion, and adoption practices;
- first-class examples for every deployment shape;
- optional OpenTelemetry integration;
- package-manager distribution beyond GitHub Release assets;
- signed installer, notarization, SBOM, and provenance workflows.

## Functionality Roadmap

Roadmap work should strengthen the execution-runner contract before expanding
surface area:

- add more real-world examples that exercise current functionality without live
  credentials by default;
- harden provider behavior through opt-in live smoke history, documented
  provider quirks, and support tiers;
- improve production workflow examples for CI jobs, queue workers, scheduled
  jobs, long-running supervisors, and failure triage;
- revisit manifest lockfile support only if it materially improves portability
  beyond `inspect --format json`;
- grow plugin ecosystem confidence through reviewed external plugins and
  registry promotion policy;
- expand local observability toward an optional OpenTelemetry bridge while
  preserving the file-based supervision contract;
- prove schema, event, trace, and CLI compatibility over multiple releases.

## Example Roadmap

Examples must demonstrate useful workflows with the current implementation.
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

## Distribution And Trust Roadmap

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

## Outside llmff

OpenClaw, Hermes, or another host owns orchestration concerns around `llmff`.
Those systems should own task decomposition, planning, memory, tool selection
policy, multi-agent coordination, human approval, long-running control loops,
global configuration, workspace indexing, provider account policy, and
task-level retry strategy.

`llmff` must not grow autonomous planning, persistent world models, global
memory, multi-agent loops, task schedulers, or host-level configuration
systems. It should remain the execution runner those systems can call, observe,
and supervise.

### v1.1 Loop And Map Stage Boundary

The v1.1 loop stage adds bounded repetition as an execution primitive. The v1.1
map stage adds bounded in-pipeline collection processing with `max_items`. These
stages do not add autonomous planning, host-level scheduling, memory, human
approval policy, or a general workflow language. Complex orchestration remains
above llmff; llmff executes the declared bounded contract and emits inspectable
artifacts.
