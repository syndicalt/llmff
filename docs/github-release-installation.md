# GitHub Release Installation

This page describes direct installation from GitHub Release assets. It is the
current supported binary distribution lane for `llmff` v0.1.4.
Package-manager publication should wait until these assets have been published,
downloaded, checksum-verified, and smoke-tested.

Set the release version once:

```bash
version=0.1.4
```

Download assets from the matching GitHub Release:

```text
https://github.com/syndicalt/llmff/releases/tag/v0.1.4
```

## Checksum Verification

Every binary archive and installer is published with an adjacent `.sha256`
file. Download both files before installing.

On Linux:

```bash
sha256sum -c llmff-${version}-x86_64-unknown-linux-gnu.tar.gz.sha256
```

On macOS:

```bash
shasum -a 256 -c llmff-${version}-aarch64-apple-darwin.tar.gz.sha256
```

On Windows PowerShell:

```powershell
$expected = (Get-Content .\llmff-0.1.4-x86_64-pc-windows-msvc.zip.sha256).Split()[0]
$actual = (Get-FileHash .\llmff-0.1.4-x86_64-pc-windows-msvc.zip -Algorithm SHA256).Hash.ToLower()
if ($actual -ne $expected) { throw "checksum mismatch" }
```

Do not install an asset if its checksum fails. Re-download it from the GitHub
Release page and verify again.

Maintainers can verify all published assets with:

```bash
scripts/check-release-assets.sh v0.1.4
```

## Linux x86_64 Archive

Download:

- `llmff-${version}-x86_64-unknown-linux-gnu.tar.gz`
- `llmff-${version}-x86_64-unknown-linux-gnu.tar.gz.sha256`

Verify and install:

```bash
sha256sum -c llmff-${version}-x86_64-unknown-linux-gnu.tar.gz.sha256
tar -xzf llmff-${version}-x86_64-unknown-linux-gnu.tar.gz
install -m 0755 llmff /usr/local/bin/llmff
llmff --version
```

The archive targets glibc Linux on x86_64. Use the Cargo source-build fallback
on other Linux architectures or libc variants.

## Debian And Ubuntu

Download:

- `llmff_${version}_amd64.deb`
- `llmff_${version}_amd64.deb.sha256`

Verify and install:

```bash
sha256sum -c llmff_${version}_amd64.deb.sha256
sudo apt install ./llmff_${version}_amd64.deb
llmff --version
```

The `.deb` is an `amd64` package for Debian, Ubuntu, and compatible
distributions.

## macOS Apple Silicon

Download either the archive or the unsigned package:

- `llmff-${version}-aarch64-apple-darwin.tar.gz`
- `llmff-${version}-aarch64-apple-darwin.tar.gz.sha256`
- `llmff-${version}-aarch64-apple-darwin.pkg`
- `llmff-${version}-aarch64-apple-darwin.pkg.sha256`

Archive install:

```bash
shasum -a 256 -c llmff-${version}-aarch64-apple-darwin.tar.gz.sha256
tar -xzf llmff-${version}-aarch64-apple-darwin.tar.gz
install -m 0755 llmff /usr/local/bin/llmff
llmff --version
```

Package install:

```bash
shasum -a 256 -c llmff-${version}-aarch64-apple-darwin.pkg.sha256
sudo installer -pkg llmff-${version}-aarch64-apple-darwin.pkg -target /
llmff --version
```

## macOS Intel

Download either the archive or the unsigned package:

- `llmff-${version}-x86_64-apple-darwin.tar.gz`
- `llmff-${version}-x86_64-apple-darwin.tar.gz.sha256`
- `llmff-${version}-x86_64-apple-darwin.pkg`
- `llmff-${version}-x86_64-apple-darwin.pkg.sha256`

Use the same archive or package commands as Apple Silicon, replacing
`aarch64-apple-darwin` with `x86_64-apple-darwin`.

## Windows x86_64

Download either the archive or the unsigned MSI:

- `llmff-${version}-x86_64-pc-windows-msvc.zip`
- `llmff-${version}-x86_64-pc-windows-msvc.zip.sha256`
- `llmff-${version}-x86_64-pc-windows-msvc.msi`
- `llmff-${version}-x86_64-pc-windows-msvc.msi.sha256`

PowerShell archive install:

```powershell
$version = "0.1.4"
$zip = "llmff-$version-x86_64-pc-windows-msvc.zip"
$expected = (Get-Content "$zip.sha256").Split()[0]
$actual = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLower()
if ($actual -ne $expected) { throw "checksum mismatch" }

Expand-Archive $zip -DestinationPath "$env:LOCALAPPDATA\llmff" -Force
& "$env:LOCALAPPDATA\llmff\llmff.exe" --version
```

MSI install:

```powershell
$msi = "llmff-0.1.4-x86_64-pc-windows-msvc.msi"
$expected = (Get-Content "$msi.sha256").Split()[0]
$actual = (Get-FileHash $msi -Algorithm SHA256).Hash.ToLower()
if ($actual -ne $expected) { throw "checksum mismatch" }

msiexec /i llmff-0.1.4-x86_64-pc-windows-msvc.msi
llmff --version
```

## Unsigned Installer Expectations

The v0.1.4 macOS `.pkg` and Windows `.msi` installers are unsigned.

- macOS: expect Gatekeeper trust prompts. Verify the `.sha256` file before
  installing. Apple Developer ID signing and notarization are planned for a
  later paid distribution track.
- Windows: expect SmartScreen or publisher warnings. Verify the `.sha256` file
  before installing. Authenticode signing is planned for a later paid
  distribution track.
- Linux: verify checksums before installing archives or `.deb` packages. The
  release does not yet publish repository metadata.

## Source-Build Fallback

Users outside the prebuilt target set can install from source:

```bash
cargo install --git https://github.com/syndicalt/llmff --tag v0.1.4 llmff
```
