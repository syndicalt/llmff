# Distribution Trust

Distribution trust is the policy for how users can verify that a package,
installer, or package-manager entry matches the release maintainers intended to
ship. GitHub Release assets are the current source of truth.

## Current Trust Baseline

The supported baseline is:

- versioned GitHub Release assets;
- adjacent SHA-256 checksum files;
- a machine-readable `llmff-<version>-release-trust.json` manifest generated
  from the staged release assets;
- local package smoke tests before release publication;
- package-manager metadata that pins immutable release URLs and expected
  SHA-256 digests;
- release docs that accurately describe unsigned installers.

This baseline is enough for early package-manager metadata readiness. The trust
manifest records asset names, sizes, SHA-256 digests, checksum sidecar
relationships, and the current checksum-only posture. It is not a complete
SBOM, signed supply-chain provenance, or OS-trusted installer identity.

## Parked Signing Tracks

Authenticode and Apple notarization stay parked until paid credentials are available.

Windows Authenticode requires a trusted code-signing certificate, protected
private-key handling, CI secret storage, revocation procedure, timestamping
policy, and recovery process before signed Windows artifacts are advertised.

Apple Developer ID signing and notarization require paid Apple Developer Program
credentials, certificate handling, notarization credentials, stapling checks,
and recovery procedures before signed macOS installers are advertised.

Unsigned artifacts may continue to ship when release notes and platform docs
state that status plainly.

## SBOM and Provenance Readiness Gate

SBOM and provenance readiness gate: before publishing llmff through a widely
used package-manager channel, maintainers must choose one of these release
postures and document it in the release checklist:

- Generate SBOM and provenance artifacts in CI, publish them next to release
  assets, and verify their presence after publication.
- Keep SBOM and provenance parked, explicitly state that the package-manager
  channel relies on release checksums plus
  `llmff-<version>-release-trust.json`, and record the follow-up work before
  broad announcement.

A generated SBOM/provenance lane must specify:

- artifact names and formats;
- build job that produces each artifact;
- checksum or signature relationship to release assets;
- post-release verification command;
- recovery steps when an artifact is missing or stale.

Until that lane exists, package-manager publication can proceed only when the
maintainers accept the checksum-only posture for that specific channel. The
release workflow generates `llmff-<version>-release-trust.json` before upload,
and `scripts/check-release-assets.sh <tag>` verifies that published release
assets include it.

## Channel Trust Requirements

Homebrew, Scoop, winget, and AUR metadata must pin release asset URLs and
SHA-256 digests. The metadata can live in the repository before publication,
but publish only after maintainers accept support ownership for updates, user
reports, rollback, and security fixes.

apt remains parked because a repository is a stronger trust commitment than a
standalone `.deb`. It needs signed repository metadata, hosting, key rotation,
historical retention, and recovery design before users are told to add a source
list entry.

## Failure Handling

If release assets, checksums, package-manager metadata, SBOM/provenance
artifacts, or signing status disagree:

- stop publication for the affected channel;
- keep the GitHub Release draft or mark the channel as blocked;
- correct the asset or metadata from a clean build;
- rerun the release asset and governance readiness checks;
- document the incident in release notes when users could have consumed the
  inconsistent artifact.
