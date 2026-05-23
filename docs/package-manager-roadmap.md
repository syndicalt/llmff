# Package Manager Roadmap

Package-manager publication starts after v0.1.3 proves that GitHub Release
assets are complete, checksum-verifiable, and smoke-tested. This page tracks
the production path without publishing anything early.

## Entry Gate

Do not submit a package-manager source until all of these are true:

- `scripts/release-preflight.sh v0.1.3` passes before the release tag is pushed.
- `scripts/check-release-assets.sh v0.1.3` passes against the published GitHub
  Release.
- Each package source downloads immutable release assets by version.
- Each package source verifies SHA-256 checksums from release metadata or embeds
  the known digest in the package definition.
- Unsigned installer expectations remain documented until paid signing is live.
- Channel maintainers explicitly decide the channel is support-ready.

Prepared metadata is not publication approval. For Homebrew, Scoop, winget, and
AUR, publish only when maintainers decide the channel is support-ready and are
prepared to handle update cadence, user reports, rollback, and security fixes.

## Homebrew Formula

Track: tap formula after GitHub Release assets are proven.

Prototype metadata:

- `packaging/homebrew/llmff.rb`
- Local validation: `scripts/check-package-manager-metadata.sh`

Target source:

- macOS Apple Silicon archive:
  `llmff-0.1.3-aarch64-apple-darwin.tar.gz`
- macOS Intel archive:
  `llmff-0.1.3-x86_64-apple-darwin.tar.gz`
- Linux x86_64 archive:
  `llmff-0.1.3-x86_64-unknown-linux-gnu.tar.gz`

Readiness work:

- Create a formula in a project-owned tap, not `homebrew-core`, until install
  volume and long-term maintenance are proven.
- Use release archive URLs, fixed version strings, and checked-in SHA-256
  values.
- Keep the formula test direct: install `bin/"llmff"` and assert
  `llmff --version` reports `llmff 0.1.3`.
- Do not submit or publish the tap before the release asset verification gate
  passes.
- Publish only when maintainers decide the channel is support-ready for tap
  ownership, formula updates, and user issue triage.

## winget

Track: submit a Windows Package Manager manifest after the unsigned MSI is
verified from the GitHub Release.

Prototype metadata:

- `packaging/winget/Syndicalt.Llmff.yaml`
- `packaging/winget/Syndicalt.Llmff.locale.en-US.yaml`
- `packaging/winget/Syndicalt.Llmff.installer.yaml`
- Local validation: `scripts/check-package-manager-metadata.sh`

Target source:

- Windows x86_64 MSI:
  `llmff-0.1.3-x86_64-pc-windows-msvc.msi`

Readiness work:

- Generate installer, default locale, and version manifests with the release
  URL and SHA-256 digest.
- Mark the installer type as `wix`.
- Keep publisher and package identifiers stable before first submission.
- Expect publisher warnings until Authenticode signing is available; do not
  imply the MSI is signed.
- Publish only when maintainers decide the channel is support-ready for Windows
  package-manager issue reports and manifest update reviews.

## Scoop

Track: add a bucket manifest after the Windows archive is verified from the
GitHub Release.

Prototype metadata:

- `packaging/scoop/llmff.json`
- Local validation: `scripts/check-package-manager-metadata.sh`

Target source:

- Windows x86_64 zip:
  `llmff-0.1.3-x86_64-pc-windows-msvc.zip`

Readiness work:

- Use the release archive URL and SHA-256 digest.
- Expose `llmff.exe` through `bin`.
- Keep `checkver` and `autoupdate` disabled until at least one manual update
  has been completed cleanly.
- Publish only after the GitHub Release archive path is stable.
- Publish only when maintainers decide the channel is support-ready for bucket
  ownership and manual update recovery.

## AUR

Track: submit an official AUR package after the generated `PKGBUILD` and
`.SRCINFO` from the release are verified.

Prototype metadata:

- `packaging/aur/PKGBUILD`
- `packaging/aur/.SRCINFO`
- Local validation: `scripts/check-package-manager-metadata.sh`

Target source:

- Linux x86_64 archive:
  `llmff-0.1.3-x86_64-unknown-linux-gnu.tar.gz`
- Release-generated metadata:
  `PKGBUILD`
  `llmff-0.1.3-arch.SRCINFO`

Readiness work:

- Review generated metadata against current AUR packaging guidelines.
- Keep the package binary-only and clearly tied to the upstream GitHub Release
  asset.
- Submit only from a maintainer account prepared to handle user comments and
  update requests.
- Do not treat generated release metadata as an official AUR submission until
  the maintainer uploads it.
- Publish only when maintainers decide the channel is support-ready for AUR
  comments, pinned checksum updates, and rollback communication.

## apt Repository

Track: evaluate feasibility after direct `.deb` installation proves reliable.

Prototype status:

- No apt repository metadata is shipped under `packaging/apt`.
- Local validation fails if unsigned apt repository metadata such as `Release`,
  `InRelease`, `Packages`, `Sources`, or `Release.gpg` is added.
- Signed repository metadata requirements are tracked in
  [`docs/apt-repository-design.md`](apt-repository-design.md).

Target source:

- Debian package:
  `llmff_0.1.3_amd64.deb`

Readiness work:

- Decide whether the project will host repository metadata, signing keys, and
  retention for historical package versions.
- Implement the signed metadata design in
  [`docs/apt-repository-design.md`](apt-repository-design.md), including
  `InRelease`, `Release.gpg`, key rotation, hosting, retention, and recovery.
- Require signed repository metadata before documenting `apt add` or
  `sources.list.d` installation.
- Keep direct `.deb` installation as the supported Debian and Ubuntu path until
  repository signing, rotation, hosting, and recovery are designed.
- Do not publish unsigned apt repository metadata.

apt stays parked until signing, repository metadata, hosting, key rotation, and recovery are designed.
Do not add apt repository instructions, repository metadata, or key-install
commands until that design is reviewed.
