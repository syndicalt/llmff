# Release Readiness

Use this checklist before advertising `llmff` as more than an early GitHub-installable project.

## Early Testing

- [x] GitHub install path is documented.
- [x] Local install path is documented.
- [x] `scripts/smoke-install.sh --path .` verifies an installed binary from an isolated Cargo home.
- [x] README examples use direct `llmff` commands.
- [x] Known MVP limitations are documented.

At this point it is reasonable to say: `llmff` is installable from GitHub for early testing.

## Broad Announcement

- [x] A versioned tag or release exists.
- [x] Fresh install has been verified from that tag or release.
- [x] Release notes describe supported platforms and known limitations.
- [x] Platform support docs describe supported CPU targets and OS assumptions.
- [x] The smoke install gate passes against the release source, not only the local checkout.
- [x] At least one end-to-end example can be run by a new user without editing repository files.
- [x] Release tag workflows publish packaged artifacts as GitHub release assets.
- [x] Release tag workflows publish native artifacts with documented unsigned Windows and macOS status.

Do not describe `llmff` as broadly released until every item in this section is checked.

Release `v0.1.1` was verified as a GitHub-installable source release with:

```bash
scripts/release-preflight.sh v0.1.1
scripts/smoke-install.sh --git https://github.com/syndicalt/llmff --tag v0.1.1
```

The next package-publication release should be cut as `v0.1.6` after this
check passes locally:

```bash
scripts/release-preflight.sh v0.1.6
```

For release tags after this packaging slice, CI creates the GitHub Release when
the tag does not already have one, then uploads binary archives, checksums,
Ubuntu/Debian packages, Arch packaging metadata, Windows MSI packages, and
macOS `.pkg` packages to the matching GitHub Release assets. Manual dispatch
keeps those outputs as Actions artifacts only.

Unsigned Windows and macOS artifacts are acceptable for v0.1.6. Windows release
tags publish an unsigned `.zip` and unsigned MSI. macOS release tags publish
unsigned `.pkg` installers. Trusted Authenticode signing, Apple Developer ID
signing, and notarization remain deferred paid distribution tracks.

Current packaged artifact targets and installer assumptions are documented in
[`docs/platform-support.md`](platform-support.md).

After release CI completes for `v0.1.6`, verify the published GitHub Release
contains the expected archive, checksum, Debian, Arch metadata, MSI, and macOS
package assets. Release publication is handled by a dependent publish job after
the full artifact matrix succeeds, so partial native-installer releases are not
published:

```bash
scripts/check-release-assets.sh v0.1.6
```

## Ecosystem Compatibility Checklist

Ecosystem compatibility checklist is required before any package-manager
publication decision.

Use this ecosystem compatibility checklist before publishing or announcing any
package-manager channel:

- [ ] `scripts/check-package-manager-metadata.sh` passes for the target release.
- [ ] `scripts/check-governance-readiness.sh` passes.
- [ ] `scripts/check-ecosystem-readiness.sh` passes, including the
  `scripts/check-agent-adoption-guide.sh` and
  `scripts/check-opentelemetry-bridge.sh` public integration gates.
- [ ] The target channel is marked support-ready by maintainers in the release
  issue or release notes.
- [ ] Homebrew, Scoop, winget, and AUR metadata pin immutable GitHub Release
  asset URLs and SHA-256 digests before publication.
- [ ] apt remains parked; no `packaging/apt` repository metadata or
  `sources.list.d` instructions are shipped.
- [ ] Unsigned Windows and macOS status is repeated in release notes unless
  Authenticode signing, Apple Developer ID signing, and notarization are live.
- [ ] SBOM/provenance posture is explicit: the release publishes
  `llmff-<version>-release-trust.json`, and maintainers either publish
  generated SBOM/provenance artifacts or record that the channel uses
  checksum-only verification for this release.
- [ ] Manifest schema, plugin protocol, CLI flags, and trace/event field changes
  are additive or have a documented deprecation path.
- [ ] Deprecated surfaces include replacement guidance, warning behavior when
  practical, release-note coverage, and removal timing.
- [ ] Provider, stage, and plugin examples still pass their focused validation
  gates and do not require live credentials for default tests.

The checklist is a release decision aid, not an automatic publication trigger.
Publishing remains parked for each channel until maintainers decide that
channel is support-ready.
