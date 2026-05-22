# llmff Roadmap

This roadmap tracks major product capabilities that move `llmff` toward an FFmpeg-shaped tool for LLM inference pipelines. Completed release notes remain under `docs/release-notes/`.

## Current Foundation

- Command-line-first pipeline runner.
- YAML manifests and compact inline graphs.
- Deterministic stages for loading, templating, system prompts, local lexical and embedding-style retrieval, local reranking, caching, routing, validation, repair, tools, and writes.
- Mock, OpenAI-compatible, and Ollama backend adapters with portable sampling, seed, JSON response-format, and stop-sequence controls. OpenAI-compatible backends also expose the first token-streaming contract.
- Dry-run inspection, JSONL traces, streamed lifecycle events, trace summaries, plugin manifest discovery, and a GitHub install smoke gate.

## Packaged Installers

Users should be able to install `llmff` without a Rust toolchain. Native packaged installers are a release-track capability for Windows, macOS, and Linux distributions, with Cargo remaining as the source-build fallback.

Target artifacts:

- Windows installer plus a signed `llmff.exe` archive.
- macOS installer or signed/notarized archive for Apple Silicon and Intel Macs.
- Ubuntu and Debian `.deb` packages.
- Arch Linux package support through an official package recipe or an AUR-ready `PKGBUILD`.
- Plain compressed binary archives for each supported platform.

Release requirements:

- Build artifacts from CI, not a developer machine.
- Publish checksums for every artifact.
- Verify each installer with a platform smoke test that runs `llmff --version`, `llmff stages list`, `llmff inspect`, and one deterministic mock-backed `llmff run`.
- Document supported CPU architectures and any libc or OS-version assumptions in `docs/platform-support.md`.
- Keep `cargo install --git ... --tag ...` available as the source-build fallback.

First implementation slice:

- `scripts/package-archive.sh` creates a compressed binary archive and adjacent SHA-256 checksum from an already-built `llmff` binary.
- `.github/workflows/release-artifacts.yml` builds release binaries on tag pushes for Linux, macOS Apple Silicon, macOS Intel, and Windows, then uploads archive artifacts.

Archive smoke implementation slice:

- `scripts/smoke-archive.sh` extracts `.tar.gz` and `.zip` release archives without installing and verifies the packaged `llmff` binary with `--version`, `stages list`, `inspect`, and a deterministic mock-backed `run`.
- `.github/workflows/release-artifacts.yml` runs the archive smoke gate for Linux, macOS Apple Silicon, macOS Intel, and Windows release archives.

Windows MSI implementation slice:

- `packaging/windows/llmff.wxs` defines a WiX installer that installs `llmff.exe` under Program Files.
- `scripts/package-windows-msi.sh` builds an unsigned x86_64 Windows MSI with WiX and writes an adjacent SHA-256 checksum.
- `.github/workflows/release-artifacts.yml` builds and checksums the Windows MSI from the Windows release-artifact job.

Windows MSI smoke implementation slice:

- `scripts/smoke-windows-msi.sh` extracts a built MSI on Windows hosts and verifies the packaged `llmff.exe` with `--version`, `stages list`, `inspect`, and a deterministic mock-backed `run`.
- `.github/workflows/release-artifacts.yml` runs the Windows MSI smoke gate from the Windows release-artifact job.

macOS PKG implementation slice:

- `scripts/package-macos-pkg.sh` builds an unsigned macOS Installer `.pkg` with `pkgbuild`, staging `llmff` into `/usr/local/bin`.
- `.github/workflows/release-artifacts.yml` builds and checksums unsigned `.pkg` installers for Apple Silicon and Intel macOS release-artifact jobs.
- Signing and notarization remain release gates before broad macOS installer recommendation.

macOS PKG smoke implementation slice:

- `scripts/smoke-macos-pkg.sh` expands a built `.pkg` on Darwin hosts and verifies the packaged `llmff` binary with `--version`, `stages list`, `inspect`, and a deterministic mock-backed `run`.
- `.github/workflows/release-artifacts.yml` runs the macOS package smoke gate for Apple Silicon and Intel macOS release-artifact jobs.

Second implementation slice:

- `scripts/package-deb.sh` creates an Ubuntu/Debian `.deb` package and adjacent SHA-256 checksum from an already-built Linux `llmff` binary.
- `.github/workflows/release-artifacts.yml` builds and inspects an `amd64` `.deb` from the Ubuntu release-artifact job.

Debian smoke implementation slice:

- `scripts/smoke-deb.sh` extracts a `.deb` without root and verifies the packaged `llmff` binary with `--version`, `stages list`, `inspect`, and a deterministic mock-backed `run`.
- `.github/workflows/release-artifacts.yml` runs the Debian package smoke gate for the Ubuntu/Debian release artifact.

Third implementation slice:

- `scripts/package-arch.sh` generates an AUR-ready `PKGBUILD` and `.SRCINFO` for the prebuilt Linux x86_64 release archive.
- `.github/workflows/release-artifacts.yml` generates and validates Arch packaging metadata from the Ubuntu release-artifact job.

Fourth implementation slice:

- `.github/workflows/release-artifacts.yml` uploads tag-built archives, checksums, `.deb` packages, and Arch metadata to the matching GitHub Release assets.
- Manual workflow dispatch keeps generated files as Actions artifacts only.

Platform support documentation slice:

- `docs/platform-support.md` describes the release target triples, artifact types, CPU architecture assumptions, Linux glibc assumption, and unsigned installer status.
- `scripts/check-platform-support-doc.sh` verifies that the documentation stays linked from the user-facing install and release-readiness docs.

## Backend Metadata

First implementation slice:

- `llmff-core` owns a typed list of built-in backend families, registration flags, mock model aliases, and capability flags.
- `llmff backends list --format json` exposes that metadata for scripts and wrappers while preserving the existing text output for humans.

Second implementation slice:

- OpenAI-compatible backend registration accepts either a server root URL or a `/v1` API root URL and resolves both to `/v1/chat/completions`.

## Stage Metadata

First implementation slice:

- `llmff-core` owns a typed list of built-in stage operations, required fields, optional fields, and capability flags.
- `llmff stages list --format json` exposes that metadata for scripts and wrappers while preserving the existing text output for humans.

## Inline Graph Expressiveness

First implementation slice:

- Inline graphs support command and HTTP tool stages with `command`, `method`, `url`, and `header:<name>` parameters.
- Command tool argv values use semicolon separators inside the `command` parameter, matching the existing inline convention for list-like values.

## Future Capability Tracks

Streaming implementation slice:

- `llmff run --events <path>` writes run and stage lifecycle events as JSONL while the pipeline executes.
- `llmff run --events -` streams those events to stdout for supervisors, dashboards, and shell pipelines.
- OpenAI-compatible backends expose a stream API that requests server-sent chat completion chunks with `stream: true`, parses content deltas, and preserves streamed usage metadata when providers emit it.
- `llmff run --stream-stage <infer-stage-id>` streams one model stage's token deltas to stdout while preserving normal manifest outputs.
- Lifecycle events and backend token streams are now separate primitives; streaming arbitrary stage payloads remains future work.

Embedding retrieval implementation slice:

- `retrieve` supports `strategy: embedding` in manifests and `strategy=embedding` in inline graphs.
- The first embedding strategy is deterministic and local, using character n-gram vectors and cosine similarity for offline retrieval without a vector database.

Rerank implementation slice:

- `rerank` accepts retrieve-shaped JSON and rescores candidates with `strategy: lexical` or `strategy: embedding`.
- The first reranker is deterministic and local, preserves candidate metadata, replaces scores, and applies optional `top_k` after sorting.
- Remote embedding providers, persistent vector indexes, and learned reranker models remain future work.

Plugin discovery implementation slice:

- `llmff-core` parses `llmff-plugin.yaml` manifests from immediate child directories of a plugin directory.
- Plugin manifests declare a name, version, and typed capabilities for `stage`, `sampler`, `backend`, or `tool-transport` extension points.
- `llmff plugins list --plugin-dir <path> --format json` exposes discovered plugin metadata for scripts and wrappers.
- `llmff run --plugin-dir <path>` can execute plugin-provided `tool-transport` capabilities as stdin/stdout command tools.
- `llmff run --plugin-dir <path>` can execute plugin-provided `stage` capabilities as stdin/stdout command stages with `op: plugin:<capability-name>`.
- `llmff run --plugin-dir <path>` can execute plugin-provided `backend` capabilities as stdin/stdout command backends.
- `llmff run --plugin-dir <path>` can execute plugin-provided `sampler` capabilities as stdin/stdout sampling override commands for `infer` and `repair` stages.

- Stronger inline graph expressiveness while keeping manifests as the canonical format for branching pipelines.
- Richer backend adapters and provider capability metadata.
- Streaming arbitrary stage outputs.
- Remote embedding-backed retrieval and learned reranking.
- More complete model/runtime abstraction once the pipeline runner is stable.
