# llmff Roadmap

This roadmap tracks major product capabilities that move `llmff` toward an FFmpeg-shaped tool for LLM inference pipelines. Completed release notes remain under `docs/release-notes/`.

## Current Foundation

- Command-line-first pipeline runner.
- YAML manifests and compact inline graphs.
- Deterministic stages for loading, templating, system prompts, local retrieval, caching, routing, validation, repair, tools, and writes.
- Mock, OpenAI-compatible, and Ollama backend adapters.
- Dry-run inspection, JSONL traces, trace summaries, and a GitHub install smoke gate.

## Packaged Installers

Users should eventually be able to install `llmff` without a Rust toolchain. This is not part of the current `v0.1.x` GitHub/Cargo install path, but it is a release-track roadmap item.

Target artifacts:

- Windows installer and signed `llmff.exe` archive.
- macOS installers or archives for Apple Silicon and Intel Macs, with signing and notarization before broad recommendation.
- Linux `.deb` packages for Ubuntu and Debian.
- Arch Linux package support, either through an official package recipe or an AUR-ready `PKGBUILD`.
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

## Future Capability Tracks

- Stronger inline graph expressiveness while keeping manifests as the canonical format for branching pipelines.
- Richer backend adapters and provider capability metadata.
- Streaming inference and streaming stage outputs.
- Embedding-backed retrieval and reranking.
- Plugin loading for stages, samplers, backends, and tool transports.
- More complete model/runtime abstraction once the pipeline runner is stable.
