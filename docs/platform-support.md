# Platform Support

This page describes the release artifacts `llmff` intends to publish and the
assumptions each artifact makes about the target system. Cargo remains the
source-build fallback for users who are outside the prebuilt artifact set.

## Supported Release Targets

| Target triple | Operating systems | Artifacts | Notes |
| --- | --- | --- | --- |
| `x86_64-unknown-linux-gnu` | Ubuntu, Debian, and compatible glibc Linux distributions on x86_64 | `.tar.gz`, `.deb`, Arch Linux `PKGBUILD` and `.SRCINFO` metadata | The prebuilt binary targets glibc Linux. The `.deb` package is built as `amd64`. Arch Linux support is metadata for the same prebuilt archive, suitable for an AUR-style package flow. |
| `aarch64-apple-darwin` | macOS on Apple Silicon | `.tar.gz`, unsigned `.pkg` | The `.pkg` installs `llmff` into `/usr/local/bin`. Release-tag CI requires Apple signing and notarization credentials before building this installer, but the package signing implementation is not wired yet. |
| `x86_64-apple-darwin` | macOS on Intel Macs | `.tar.gz`, unsigned `.pkg` | The `.pkg` installs `llmff` into `/usr/local/bin`. Release-tag CI requires Apple signing and notarization credentials before building this installer, but the package signing implementation is not wired yet. |
| `x86_64-pc-windows-msvc` | 64-bit Windows | `.zip`, signed `.msi` on release tags, unsigned `.msi` on manual dispatch | The MSI is built with WiX on a Windows runner and installs `llmff.exe` under Program Files. Release-tag CI signs the MSI with Authenticode, verifies the signature, regenerates the checksum, and smoke-tests the signed installer payload. |

## Installer Status

- Ubuntu and Debian: CI builds an `amd64` `.deb`, publishes an adjacent SHA-256
  checksum, and verifies it without root using `scripts/smoke-deb.sh`.
- Arch Linux: CI generates `PKGBUILD` and `.SRCINFO` metadata for the Linux
  x86_64 release archive. This is not yet an official repository package.
- macOS: CI builds unsigned `.pkg` installers for Apple Silicon and Intel Macs,
  publishes checksums, and expands the package payload with
  `scripts/smoke-macos-pkg.sh`. Tag-triggered release CI first runs
  `scripts/check-release-signing-gates.sh --platform macos` so missing Apple
  signing or notarization credentials block release installer publication.
- Windows: CI builds an x86_64 MSI on a Windows host. Manual workflow dispatch
  keeps the MSI unsigned for packaging tests. Tag-triggered release CI first
  runs `scripts/check-release-signing-gates.sh --platform windows`, then signs
  the MSI with `scripts/sign-windows-msi.ps1`, verifies the Authenticode
  signature with `signtool`, regenerates the checksum, and extracts the signed
  MSI payload with `scripts/smoke-windows-msi.sh`.
- Archives: CI builds `.tar.gz` archives for Linux and macOS and a `.zip`
  archive for Windows. `scripts/smoke-archive.sh` extracts each archive and
  runs the packaged binary.

## Verification Gates

Every packaged binary smoke gate exercises the same CLI surface:

- `llmff --version`
- `llmff stages list`
- `llmff inspect examples/json-repair.yaml --mock llmff:good`
- one deterministic mock-backed `llmff run`

The current gates cover raw archives, Debian packages, macOS package payloads,
Windows MSI payloads, tag-only signing credential preflights, and Windows MSI
Authenticode signing. The project should not treat macOS native installers as
broadly recommended until package signing and notarization commands are wired.

Before creating or pushing a release tag, run the metadata preflight:

```bash
scripts/release-preflight.sh v0.1.1
```

## Source-Build Fallback

Users with a Rust toolchain can install from a tagged release:

```bash
cargo install --git https://github.com/syndicalt/llmff --tag v0.1.1 llmff
```

That path remains supported even after native installers are published.
