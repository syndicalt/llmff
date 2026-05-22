# Platform Support

This page describes the release artifacts `llmff` intends to publish and the
assumptions each artifact makes about the target system. Cargo remains the
source-build fallback for users who are outside the prebuilt artifact set.

## Supported Release Targets

| Target triple | Operating systems | Artifacts | Notes |
| --- | --- | --- | --- |
| `x86_64-unknown-linux-gnu` | Ubuntu, Debian, and compatible glibc Linux distributions on x86_64 | `.tar.gz`, `.deb`, Arch Linux `PKGBUILD` and `.SRCINFO` metadata | The prebuilt binary targets glibc Linux. The `.deb` package is built as `amd64`. Arch Linux support is metadata for the same prebuilt archive, suitable for an AUR-style package flow. |
| `aarch64-apple-darwin` | macOS on Apple Silicon | `.tar.gz`, unsigned `.pkg` | The `.pkg` installs `llmff` into `/usr/local/bin`. It is not signed or notarized yet. |
| `x86_64-apple-darwin` | macOS on Intel Macs | `.tar.gz`, unsigned `.pkg` | The `.pkg` installs `llmff` into `/usr/local/bin`. It is not signed or notarized yet. |
| `x86_64-pc-windows-msvc` | 64-bit Windows | `.zip`, unsigned `.msi` | The MSI is built with WiX on a Windows runner and installs `llmff.exe` under Program Files. It is not Authenticode signed yet. |

## Installer Status

- Ubuntu and Debian: CI builds an `amd64` `.deb`, publishes an adjacent SHA-256
  checksum, and verifies it without root using `scripts/smoke-deb.sh`.
- Arch Linux: CI generates `PKGBUILD` and `.SRCINFO` metadata for the Linux
  x86_64 release archive. This is not yet an official repository package.
- macOS: CI builds unsigned `.pkg` installers for Apple Silicon and Intel Macs,
  publishes checksums, and expands the package payload with
  `scripts/smoke-macos-pkg.sh`.
- Windows: CI builds an unsigned x86_64 MSI and checksum on a Windows host.
  Signing and a full installed-MSI smoke gate are still release-track gaps.
- Archives: CI builds `.tar.gz` archives for Linux and macOS and a `.zip`
  archive for Windows. `scripts/smoke-archive.sh` extracts each archive and
  runs the packaged binary.

## Verification Gates

Every packaged binary smoke gate exercises the same CLI surface:

- `llmff --version`
- `llmff stages list`
- `llmff inspect examples/json-repair.yaml --mock llmff:good`
- one deterministic mock-backed `llmff run`

The current gates cover raw archives, Debian packages, and macOS package
payloads. Windows MSI packaging is built and checksummed in CI, but the project
should not treat the MSI as broadly recommended until an installed-MSI smoke
gate and code signing are in place.

## Source-Build Fallback

Users with a Rust toolchain can install from a tagged release:

```bash
cargo install --git https://github.com/syndicalt/llmff --tag v0.1.1 llmff
```

That path remains supported even after native installers are published.
