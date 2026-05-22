# llmff Roadmap

This roadmap tracks major product capabilities that move `llmff` toward an FFmpeg-shaped tool for LLM inference pipelines. Completed release notes remain under `docs/release-notes/`.

## Current Foundation

- Command-line-first pipeline runner.
- YAML manifests and compact inline graphs.
- Deterministic stages for loading, templating, system prompts, local retrieval, caching, routing, validation, repair, tools, and writes.
- Mock, OpenAI-compatible, and Ollama backend adapters.
- Dry-run inspection, JSONL traces, trace summaries, and a GitHub install smoke gate.

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
- Document supported CPU architectures and any libc or OS-version assumptions.
- Keep `cargo install --git ... --tag ...` available as the source-build fallback.

First implementation slice:

- `scripts/package-archive.sh` creates a compressed binary archive and adjacent SHA-256 checksum from an already-built `llmff` binary.
- `.github/workflows/release-artifacts.yml` builds release binaries on tag pushes for Linux, macOS Apple Silicon, macOS Intel, and Windows, then uploads archive artifacts.

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

- Stronger inline graph expressiveness while keeping manifests as the canonical format for branching pipelines.
- Richer backend adapters and provider capability metadata.
- Streaming inference and streaming stage outputs.
- Embedding-backed retrieval and reranking.
- Plugin loading for stages, samplers, backends, and tool transports.
- More complete model/runtime abstraction once the pipeline runner is stable.
