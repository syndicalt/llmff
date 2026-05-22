# Platform Support

This page describes the release artifacts `llmff` intends to publish and the
assumptions each artifact makes about the target system. Cargo remains the
source-build fallback for users who are outside the prebuilt artifact set.

## Supported Release Targets

| Target triple | Operating systems | Artifacts | Notes |
| --- | --- | --- | --- |
| `x86_64-unknown-linux-gnu` | Ubuntu, Debian, and compatible glibc Linux distributions on x86_64 | `.tar.gz`, `.deb`, Arch Linux `PKGBUILD` and `.SRCINFO` metadata | The prebuilt binary targets glibc Linux. The `.deb` package is built as `amd64`. Arch Linux support is metadata for the same prebuilt archive, suitable for an AUR-style package flow. |
| `aarch64-apple-darwin` | macOS on Apple Silicon | `.tar.gz`, unsigned `.pkg` | The `.pkg` installs `llmff` into `/usr/local/bin`. Apple Developer ID signing and notarization remain a future paid distribution track. |
| `x86_64-apple-darwin` | macOS on Intel Macs | `.tar.gz`, unsigned `.pkg` | The `.pkg` installs `llmff` into `/usr/local/bin`. Apple Developer ID signing and notarization remain a future paid distribution track. |
| `x86_64-pc-windows-msvc` | 64-bit Windows | unsigned `.zip` and unsigned `.msi` | The Windows archive contains `llmff.exe`. The MSI is built with WiX on a Windows runner and installs `llmff.exe` under Program Files. Windows Authenticode signing remains a future paid distribution track. |

## Installer Status

- Ubuntu and Debian: CI builds an `amd64` `.deb`, publishes an adjacent SHA-256
  checksum, and verifies it without root using `scripts/smoke-deb.sh`.
- Arch Linux: CI generates `PKGBUILD` and `.SRCINFO` metadata for the Linux
  x86_64 release archive. This is not yet an official repository package.
- macOS: CI builds unsigned `.pkg` installers for Apple Silicon and Intel Macs,
  publishes adjacent SHA-256 checksums, and expands each package payload with
  `scripts/smoke-macos-pkg.sh`. Apple Developer ID signing and notarization are
  deferred until paid Apple Developer Program credentials are available.
- Windows: CI builds an unsigned `.zip` archive and unsigned x86_64 MSI on a
  Windows host, publishes adjacent SHA-256 checksums, and extracts the MSI
  payload with `scripts/smoke-windows-msi.sh`. Authenticode signing is deferred
  until a trusted code-signing certificate is available.
- Archives: CI builds `.tar.gz` archives for Linux and macOS and a `.zip`
  archive for Windows. `scripts/smoke-archive.sh` extracts each archive and
  runs the packaged binary.

## Verification Gates

Every packaged binary smoke gate exercises the same CLI surface:

- `llmff --version`
- `llmff stages list`
- `llmff inspect examples/json-repair.yaml`
- one deterministic mock-backed `llmff run`

The current gates cover raw archives, Debian packages, macOS package payloads,
Windows MSI payloads, unsigned release publication, and deferred signing helper
wiring.

Before creating or pushing a release tag, run the metadata preflight:

```bash
scripts/release-preflight.sh v0.1.2
```

After release CI finishes, verify the published GitHub Release assets from a
host that can run at least one packaged artifact:

```bash
scripts/check-release-assets.sh v0.1.2
```

## Source-Build Fallback

Users with a Rust toolchain can install from a tagged release:

```bash
cargo install --git https://github.com/syndicalt/llmff --tag v0.1.2 llmff
```

That path remains supported even after native installers are published.
