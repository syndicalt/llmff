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

The immediate release goal is `v0.1.2`: the first release cut after packaged
artifact publication landed, with Windows and macOS artifacts explicitly
published unsigned.

Required before broad native-installer announcement:

- Run `scripts/release-preflight.sh v0.1.2` on the tag candidate.
- Push the `v0.1.2` tag and verify GitHub Actions creates the release.
- Confirm release assets include Linux/macOS/Windows archives, checksums,
  Debian package, Arch metadata, Windows MSI, and macOS `.pkg` files.
- Run `scripts/check-release-assets.sh v0.1.2` to verify published assets,
  checksums, and host-compatible package smoke tests.
- Smoke test at least the source-build install path from the published tag.
- Smoke test native packages on their target platforms before describing them
  as broadly verified.

Trusted signing and notarization remain a future paid distribution track:

- Windows Authenticode signing remains a future paid distribution track.
- Apple Developer ID signing and notarization remain a future paid distribution track.

## Next Product Roadmap

Provider onboarding:

- Add runnable OpenAI-compatible and Ollama examples that pair manifests with
  documented environment variables and mock fallbacks.
- Add provider troubleshooting docs for API key lookup, base URL normalization,
  JSON response-format support, token streaming support, and common HTTP
  failure modes.
- Add reusable manifest templates for summarization, structured extraction,
  JSON repair, retrieve-rerank-answer, and tool-call workflows.

Streaming and supervision:

- Add a documented event schema reference with compatibility expectations for
  supervisors and dashboards.
- Add a CLI smoke fixture that exercises `--events -`, `--events <path>`, and
  `--stream-stage` against deterministic mock and streaming backend paths.
- Add examples for piping lifecycle events into common shell tooling without
  interleaving stage payload output.

Plugin ecosystem:

- Add a plugin author guide covering manifest schema, command protocol,
  working-directory expectations, stdin/stdout JSON contracts, and security
  boundaries.
- Add example plugins for one stage, one backend, one sampler, and one tool
  transport, each covered by CLI smoke tests.
- Add plugin validation diagnostics that identify malformed manifests and
  missing entrypoints without requiring a pipeline run.

Distribution:

- Add Homebrew formula, winget/Scoop manifest, official AUR submission, and
  apt repository feasibility tracks after `v0.1.2` proves GitHub Release assets.
- Document per-platform installation from GitHub Release assets, including
  checksum verification and installer trust expectations.
