# llmff Roadmap

This roadmap tracks major product capabilities that move `llmff` toward an FFmpeg-shaped tool for LLM inference pipelines. Completed release notes remain under `docs/release-notes/`.

## Current Foundation

- Command-line-first pipeline runner.
- YAML manifests and compact inline graphs.
- Deterministic stages for loading, templating, system prompts, local lexical and embedding-style retrieval, local reranking, caching, routing, validation, repair, tools, and writes.
- Mock, OpenAI-compatible, and Ollama backend adapters with portable sampling, seed, JSON response-format, and stop-sequence controls. OpenAI-compatible backends also expose the first token-streaming contract.
- Dry-run inspection, JSONL traces, streamed lifecycle events, trace summaries, plugin manifest discovery, and a GitHub install smoke gate.

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

## Next Product Roadmap

Provider onboarding:

- Add provider-specific live smoke jobs in CI once secret policy and runner
  expectations are settled.
- Add provider examples for additional OpenAI-compatible gateways.

Streaming and supervision:

- Add machine-readable event schema fixtures for downstream supervisor tests.
- Add richer failure classification only when a concrete consumer needs it.

Plugin ecosystem:

- Add plugin protocol fixtures that third-party plugin authors can run in their
  own CI.
- Add more example plugins once real extension use cases emerge.

Distribution:

- Design signed apt repository metadata before documenting apt repository
  installation.
- Publish package-manager metadata only after maintainers decide each channel is
  ready for support.
