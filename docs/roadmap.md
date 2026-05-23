# llmff Roadmap

This roadmap tracks major product capabilities that move `llmff` toward an FFmpeg-shaped tool for LLM inference pipelines. Completed release notes remain under `docs/release-notes/`.

## Current Foundation

- Command-line-first pipeline runner.
- YAML manifests and compact inline graphs.
- Deterministic stages for loading, templating, system prompts, local lexical and embedding-style retrieval, local reranking, caching, routing, validation, repair, tools, and writes.
- Mock, OpenAI-compatible, and Ollama backend adapters with portable sampling, seed, JSON response-format, and stop-sequence controls. OpenAI-compatible backends also expose the first token-streaming contract.
- Dry-run inspection with text and JSON reproducibility reports, JSONL traces,
  streamed lifecycle events, trace summaries, plugin manifest discovery, and a
  GitHub install smoke gate.

## Completed Release Tracks

The first packaged-artifact track is implemented on `main`. Release tags build
artifacts from CI, publish checksums, run package smoke tests, and keep Cargo
as the source-build fallback.

Implemented artifact targets:

- Windows installer plus an unsigned `llmff.exe` archive.
- macOS unsigned `.pkg` installers for Apple Silicon and Intel Macs.
- Ubuntu and Debian `.deb` packages.
- Arch Linux package support through an official package recipe or an AUR-ready `PKGBUILD`.
- Plain compressed binary archives for each supported platform.

Implemented release gates:

- CI-built release archives for Linux, macOS Apple Silicon, macOS Intel, and Windows.
- Archive, Debian package, macOS package payload, and Windows MSI smoke tests.
- Unsigned Windows `.zip` archives and unsigned Windows MSI packages.
- Unsigned macOS `.pkg` installers.
- GitHub Release creation and asset upload for tag-triggered release jobs.
- Platform support documentation and local release preflight checks.

## Completed Metadata And Runtime Tracks

The metadata and runtime discovery slices are implemented on `main`:

- `llmff stages list --format json` exposes built-in stage operations, required fields, optional fields, and capability flags.
- `llmff backends list --format json` exposes built-in and runtime-registered backend family metadata for mock, OpenAI-compatible, Ollama, and plugin command backends.
- OpenAI-compatible backend registration accepts server root URLs and `/v1` API root URLs.
- Inline graphs support command/HTTP tool stages, named stages, and `from=<id>` references.
- `llmff plugins list --plugin-dir <path> --format json` discovers plugin manifests.
- `llmff run --plugin-dir <path>` can execute plugin-provided tool transports, stages, backends, and sampler overrides.
- `llmff models list --format json` exposes runtime model aliases for built-in mock models, CLI-registered OpenAI-compatible/Ollama aliases, and plugin command backends.

## Completed Pipeline Capability Tracks

The first pipeline execution, retrieval, streaming, and extension tracks are
implemented on `main`:

- `llmff run --events <path>` writes lifecycle events as JSONL while the pipeline executes.
- `llmff run --events -` streams lifecycle events to stdout for supervisors, dashboards, and shell pipelines.
- OpenAI-compatible backends expose the first token-streaming API contract.
- `llmff run --stream-stage <stage-id>` streams one selected stage to stdout.
- `retrieve` supports lexical, deterministic local embedding, command-provider retrieval, and persistent local embedding indexes.
- `rerank` supports deterministic local lexical/embedding reranking and command-provider reranking.

## Release Stabilization

The `v0.1.2` package-publication release is complete. The published GitHub
Release carries Linux, macOS, and Windows archives, checksums, Debian package
assets, Arch package metadata, Windows MSI, and macOS `.pkg` installers.

Required before broad native-installer announcement:

- Run `scripts/check-release-assets.sh v0.1.2` against the published GitHub
  Release from a host that can smoke-test at least one native artifact.
- Smoke test native packages on their target platforms before describing them
  as broadly verified.

Trusted signing and notarization remain a future paid distribution track:

- Windows Authenticode signing remains a future paid distribution track.
- Apple Developer ID signing and notarization remain a future paid distribution track.

## Completed Product Sprint

Provider onboarding:

- Runnable OpenAI-compatible and Ollama examples pair manifests with documented
  environment variables and mock fallbacks.
- Provider troubleshooting docs cover API key lookup, base URL normalization,
  JSON response-format support, token streaming support, and common HTTP
  failure modes.
- Reusable manifest templates cover summarization, structured extraction,
  JSON repair, retrieve-rerank-answer, and tool-call workflows.

Streaming and supervision:

- The event schema reference documents compatibility expectations for
  supervisors and dashboards.
- The CLI smoke fixture exercises `--events -`, `--events <path>`, and
  `--stream-stage` against deterministic mock and streaming backend paths.
- Streaming examples show piping lifecycle events into shell tooling without
  interleaving stage payload output.

Plugin ecosystem:

- The plugin author guide covers manifest schema, command protocol,
  working-directory expectations, stdin/stdout JSON contracts, and security
  boundaries.
- Example plugins cover one stage, one backend, one sampler, and one tool
  transport, each covered by CLI smoke tests.
- `llmff plugins validate` identifies malformed manifests and
  missing entrypoints without requiring a pipeline run.

Distribution:

- Per-platform installation from GitHub Release assets is documented, including
  checksum verification and installer trust expectations.
- Homebrew formula, winget, Scoop, official AUR submission, and apt repository
  feasibility tracks are documented for post-release package-manager work.

## Completed Next Roadmap Sprint

Provider onboarding:

- Opt-in live smoke scripts cover OpenAI-compatible and Ollama provider paths
  without requiring credentials or local services in default tests.
- Reusable templates now cover multi-step extraction and batch processing.

Streaming and supervision:

- `run_failed` lifecycle events provide a sanitized failure contract with stable
  `failure_kind` and `failure_message` fields.
- Supervisor examples cover long-running process handling and parallel
  execution event consumption.

Plugin ecosystem:

- `llmff plugins validate --format json` emits structured validation reports
  for machine consumers.
- Plugin protocol v1 and validation report v1 compatibility expectations are
  documented.

Distribution:

- Homebrew, winget, Scoop, and AUR package-manager metadata prototypes are
  produced and locally validated without publishing.
- Apt repository metadata remains parked until signing, key management,
  hosting, rotation, and recovery are designed.

## Completed Mature Ecosystem Sprint

Core contract:

- Pipeline manifest schema v1, trace schema v1, event schema v1, plugin
  manifest schema v1, and plugin validation report schema v1 are published
  under `docs/schemas/`.
- Inline graph syntax is documented as syntax version `1` through manifest
  metadata.
- Golden fixtures cover compatible manifests, successful and failed event
  streams, trace records, plugin manifests, and plugin validation reports.
- `scripts/check-schema-contract.py` and the schema contract integration test
  validate the machine-readable contracts.

Provider confidence:

- Provider examples now cover OpenAI, Azure OpenAI, LM Studio, vLLM, LocalAI,
  OpenRouter, Together, Groq, and Anthropic-compatible adapter usage.
- `.github/workflows/live-provider-smoke.yml` provides opt-in live OpenAI-
  compatible and Ollama smoke jobs without running on pull requests by default.
- `llmff backends report` emits provider capability reports for JSON mode,
  streaming, seed, stop sequences, usage metadata, API key configuration, and
  backend diagnostics.

Plugin ecosystem:

- Plugin protocol v1 fixtures are published under
  `docs/plugins/fixtures/protocol-v1/`.
- Official example plugins now cover retrieval provider, reranker, model
  backend, sampler, tool transport, and postprocessor-as-stage patterns.
- A static plugin registry format and trust review guidance are documented.

Pipeline library:

- Production-oriented templates cover summarization, extraction,
  classification, JSON repair, RAG answer, batch processing, tool calling, eval
  harness, multi-provider fallback, and cost/latency comparison.
- Template catalog tests inspect every documented template.
- `docs/pipeline-library.md` provides copy-and-run commands for each template.

Execution maturity:

- Failure classification includes safe `run_failed` categories for backend,
  HTTP, timeout, graph, config, schema, and stage failures.
- Model and HTTP tool stages support retry/backoff policies.
- Per-stage and default timeouts, concurrency limits, batch input mode,
  checkpoint/resume, cache refresh/bypass/read policies, and guarded
  trace-replay validation are implemented and tested.

Observability and supervision:

- Event schema fixtures, supervisor/dashboard examples, and trace-to-summary
  plus trace-to-metrics exporters are published.
- Exporters summarize run wall-clock duration, stage timing, token usage, cache
  hit rate, backend error rate, timeout rate, and failure counts. The metrics
  exporter is the local hook for future OpenTelemetry bridges.

Distribution and trust:

- Homebrew, Scoop, winget, and AUR metadata remain validated and support-ready
  only when maintainers decide to publish each channel.
- Apt remains parked until signing, repository metadata, hosting, key rotation,
  and recovery are designed.
- Authenticode and Apple notarization remain parked until paid credentials are
  available.
- SBOM/provenance posture and release trust checks are documented.

Governance:

- Stability, contribution, release compatibility, and deprecation policies now
  cover manifest schema, plugin protocol, CLI flags, and trace/event fields.
- `scripts/check-governance-readiness.sh` validates governance readiness docs.

## Next Product Roadmap

The next roadmap tracks move `llmff` toward a dependable FFmpeg-style runner
for LLM inference pipelines: boring process semantics, explicit inputs,
reproducible manifests, inspectable execution, and stable machine-readable
outputs. For agent systems, `llmff` should be a bounded execution tool: the
agent decides what needs to happen, and `llmff` performs the pipeline work in a
way that is observable, restartable, and easy to supervise.

### Execution Contract Hardening

Goal: make `llmff run` fully dependable as a subprocess primitive.

- Keep exit codes stable and documented across successful runs, stage failures,
  graph/configuration failures, batch failures, and interrupted runs. Initial
  stable process codes are implemented for success, CLI usage failures,
  validation failures, stage failures, provider/runtime failures, local data
  failures, and intentionally unsupported behavior.
- Keep stdout, stderr, `--events`, `--trace`, `--stream-stage`, and manifest
  outputs unambiguous so supervisors can safely compose `llmff` in shells,
  daemons, and agent runtimes.
- Strengthen checkpoint/resume semantics for interrupted and long-running jobs,
  including clearer operator diagnostics when a checkpoint cannot be reused.
  Checkpoint manifest mismatches now report the checkpoint path, saved hash,
  current manifest hash, and inspect hint.
- Expand failure classification only through additive `failure_kind` values
  with schema fixtures and compatibility notes.
- Add focused contract tests for process behavior that external supervisors are
  expected to rely on.

### Agent Embedding Surface

Goal: make it trivial for agents to call `llmff` safely.

- Promote `docs/agent-workflows.md` into a complete embedding guide with
  canonical subprocess patterns for short jobs, long jobs, batch jobs, and
  streaming jobs.
- The Python supervisor reference now performs inspect JSON preflight, runs the
  deterministic offline pipeline, captures events, writes trace/checkpoint
  artifacts, and preserves the `llmff` process exit code.
- Add more reference integrations for common agent host languages only where
  the integration contract is genuinely different from the Python subprocess
  pattern.
- Document retry, timeout, checkpoint, trace, event, and artifact ownership
  patterns for agent supervisors.
- Provide copyable examples that show how an agent should interpret
  `run_failed.failure_kind`, preserve the exit code, and avoid reading prompt
  payloads from metadata streams.
- Keep examples offline-runnable by default, with live-provider variants gated
  behind explicit secrets and opt-in flags.

### Manifest Reproducibility

Goal: make manifests portable, auditable, and predictable before execution.

- Improve `llmff inspect` output so operators can see resolved inputs, outputs,
  stage order, backend aliases, model ids, plugin dependencies, cache policy,
  checkpoint/resume policy, stdout/artifact ownership, requested execution
  controls, and known capability constraints before a run.
- Reproducibility reports now summarize the manifest hash, schema version,
  inline graph syntax version, backend registrations, plugin protocol version,
  plugin manifests, stage capability constraints, and execution controls.
- Keep all inputs and outputs explicit: file inputs, stdin, batch input,
  generated artifacts, and stdout-producing stages should be visible in
  inspection output.
- Explore lockfile or manifest-lock support only if it materially improves
  portability across machines and provider configurations.
- Maintain schema compatibility fixtures for every additive manifest contract
  change.

### Observability And Supervision

Goal: make every run inspectable after the fact and monitorable while running.

- Local exporter slices are implemented on `main`: trace summaries include run
  duration, per-stage timing, retry counts, timeout status, cache behavior,
  token usage, backend diagnostics, output artifact locations, and sanitized
  failure breakdowns.
- Keep event and trace schemas stable, additive, and backed by fixtures that
  downstream dashboards can use as compatibility tests.
- Improve local exporters while keeping telemetry local-first and opt-in:
  no collectors, network calls, or external services by default.
- Define a clear bridge point for future OpenTelemetry integration without
  changing the current file-based supervision contract.
- Same-run observability examples now demonstrate live event consumption,
  post-run summaries, and metrics export from one local execution.

### Provider And Plugin Confidence

Goal: make external integrations boring.

- Run live provider smoke jobs only once maintainers have decided which secrets
  and runner expectations are supportable.
- Keep provider capability reports focused on what manifests and supervisors
  need to know: JSON mode, streaming, seed, stop sequences, usage metadata,
  authentication, and known diagnostics.
- Use plugin protocol fixtures with third-party plugin authors and promote real
  extensions into the static registry only after review.
- Plugin validation reports now include static conformance checks for command
  protocol coverage, schema output expectations, error handling expectations,
  executable entrypoints, and trust-boundary review without requiring a full
  pipeline run.
- Preserve process isolation and explicit trust boundaries for plugin execution;
  do not imply sandboxing unless it exists.

### Distribution And Trust

Goal: make installation, verification, and upgrades predictable.

- Keep GitHub Release assets, checksums, and local release verification as the
  default supported distribution lane.
- Release publication now generates `llmff-<version>-release-trust.json` from
  staged assets so checksum-only trust posture is machine-readable. Full
  SBOM/provenance artifacts remain a future support commitment if release
  adoption justifies them.
- Keep Homebrew, Scoop, winget, and AUR metadata support-ready but unpublished
  until maintainers explicitly decide each channel is supportable.
- Design signed apt repository metadata before documenting apt repository
  installation.
- Keep Authenticode signing, Apple Developer ID signing, and notarization
  parked until paid credentials and recovery procedures are available.

### Ecosystem Readiness

Goal: let other tools build on `llmff` without relying on internal knowledge.

- Maintain stable schemas, fixtures, and compatibility policy for manifests,
  traces, events, plugin manifests, validation reports, and CLI JSON output.
- Keep roadmap, release notes, provider docs, plugin docs, and agent workflow
  examples aligned with the supported contract.
- Add adoption-oriented guides only when they demonstrate real integration
  patterns instead of duplicating CLI reference material.
- Treat registry promotion, package-manager publication, and live-provider
  certification as support commitments, not just generated metadata.
- Keep every public integration path covered by a local validation gate or a
  documented opt-in live smoke gate.
