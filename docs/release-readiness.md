# Release Readiness

Use this checklist before advertising `llmff` as more than an early GitHub-installable project.

For `v1.0.0`, release readiness also requires the contract audit in
[`docs/v1-contract.md`](v1-contract.md) and the operational evidence in
[`docs/release-runbook.md`](release-runbook.md). Do not describe a surface as
stable in release notes, docs, package metadata, or launch material unless it is
classified as `stable-for-1.0` or explicitly promoted during the release
review.

## Early Testing

- [x] GitHub install path is documented.
- [x] Local install path is documented.
- [x] `scripts/smoke-install.sh --path .` verifies an installed binary from an isolated Cargo home.
- [x] README examples use direct `llmff` commands.
- [x] Known MVP limitations are documented.

At this point it is reasonable to say: `llmff` is installable from GitHub for early testing.

## Historical Broad Announcement Baseline

- [x] A versioned tag or release exists.
- [x] Fresh install has been verified from that tag or release.
- [x] Release notes describe supported platforms and known limitations.
- [x] Platform support docs describe supported CPU targets and OS assumptions.
- [x] The smoke install gate passes against the release source, not only the local checkout.
- [x] At least one end-to-end example can be run by a new user without editing repository files.
- [x] Release tag workflows publish packaged artifacts as GitHub release assets.
- [x] Release tag workflows publish native artifacts with documented unsigned Windows and macOS status.

The checked items in this section describe the already-completed early release
baseline, not the active `v0.8.0` or `v1.0.0` release train. Do not describe a
new release as broadly released until its own tag, CI artifacts, install smoke,
and asset verification have passed.

Release `v0.1.1` was verified as a GitHub-installable source release with:

```bash
scripts/release-preflight.sh v0.1.1
scripts/smoke-install.sh --git https://github.com/syndicalt/llmff --tag v0.1.1
```

The v1.0 release-candidate train starts at `v0.8.0`. Cut `v0.8.0` only after
this check passes locally:

```bash
scripts/release-preflight.sh v0.8.0
```

Cut the final `v1.0.0` tag only after the compatibility review and this local
preflight pass:

```bash
scripts/release-preflight.sh v1.0.0
```

Cut the `v1.1.0` tag only after the loop-stage compatibility review and this
local preflight pass:

```bash
scripts/release-preflight.sh v1.1.0
```

After release CI completes for `v1.1.0`, verify the published GitHub Release
assets with:

```bash
scripts/check-release-assets.sh v1.1.0
```

Cut the `v1.2.0` tag only after the declared multi-agent topology review and
this local preflight pass:

```bash
scripts/release-preflight.sh v1.2.0
```

After release CI completes for `v1.2.0`, verify the published GitHub Release
assets with:

```bash
scripts/check-release-assets.sh v1.2.0
```

For release tags in the release-candidate train, CI creates the GitHub Release when
the tag does not already have one, then uploads binary archives, checksums,
Ubuntu/Debian packages, Arch packaging metadata, Windows MSI packages, and
macOS `.pkg` packages to the matching GitHub Release assets. Manual dispatch
keeps those outputs as Actions artifacts only.

Unsigned Windows and macOS artifacts are acceptable for v1.2.0. The
parked-signing policy — what ships unsigned today and what unparking
Authenticode and Apple notarization requires — is in
[`docs/distribution-trust.md`](distribution-trust.md#parked-signing-tracks).

Current packaged artifact targets and installer assumptions are documented in
[`docs/platform-support.md`](platform-support.md).

After release CI completes for the final `v1.0.0` tag, verify the published GitHub Release
contains the expected archive, checksum, Debian, Arch metadata, MSI, and macOS
package assets. Release publication is handled by a dependent publish job after
the full artifact matrix succeeds, so partial native-installer releases are not
published:

```bash
scripts/check-release-assets.sh v1.0.0
```

The exact release-candidate and final-release evidence requirements are in
[`docs/release-runbook.md`](release-runbook.md). Local artifact preparation does
not complete the release-candidate step until the tag exists, release-tag CI
passes, published assets verify, and the GitHub install smoke passes for that
tag.

## Ecosystem Compatibility Checklist

Ecosystem compatibility checklist is required before any package-manager
publication decision.

Use this ecosystem compatibility checklist before publishing or announcing any
package-manager channel:

- [ ] `scripts/check-package-manager-metadata.sh` passes for the exact package
  metadata version being published. If the active release has no published
  package-manager channel, leave the checked-in metadata parked rather than
  inventing hashes for unpublished assets.
- [ ] `scripts/check-governance-readiness.sh` passes.
- [ ] `scripts/check-ecosystem-readiness.sh` passes, including the
  `scripts/check-agent-adoption-guide.sh` and
  `scripts/check-opentelemetry-bridge.sh` public integration gates.
- [ ] The target channel is marked support-ready by maintainers in the release
  issue or release notes.
- [ ] Homebrew, Scoop, winget, and AUR metadata publication follows the
  channel trust requirements in
  [`docs/distribution-trust.md`](distribution-trust.md#channel-trust-requirements).
- [ ] apt remains parked; no `packaging/apt` repository metadata or
  `sources.list.d` instructions are shipped.
- [ ] Release notes repeat the unsigned-artifact status per
  [`docs/distribution-trust.md`](distribution-trust.md#parked-signing-tracks).
- [ ] SBOM/provenance posture follows the readiness gate in
  [`docs/distribution-trust.md`](distribution-trust.md#sbom-and-provenance-readiness-gate).
- [ ] Manifest, plugin, CLI, and trace/event changes follow the additive and
  deprecation policy in
  [`docs/governance.md`](governance.md#deprecation-policy).
- [ ] Provider, stage, and plugin examples still pass their focused validation
  gates and do not require live credentials for default tests.

The checklist is a release decision aid, not an automatic publication
trigger; see [`docs/governance.md`](governance.md#decision-owners) for who
decides a channel is support-ready.

## V0.8 API Freeze

`v0.8.0` is the API-freeze release for the v1.0 train. After this point, do not
add public CLI flags, manifest fields, plugin protocol behavior, trace/event
fields, run-directory artifacts, or Rust library exports unless the change is
required to fix a v1.0 blocker, security issue, or compatibility bug found
during release-candidate validation.

Release-candidate validation should use at least one `v0.8.x` tag to verify
install, release artifacts, documentation, examples, provider setup, plugin
template adoption, and external supervisor integrations before `v1.0.0`.

## V1.0 Contract Freeze Checklist

Use this checklist before cutting a `v1.0.0` release candidate:

- [ ] Required pull-request CI is green on the release branch, including
  formatting, clippy, locked workspace tests, schema, plugin, governance,
  ecosystem, real-world workflow, and event-streaming smoke gates.
- [ ] Release-candidate evidence required by `docs/release-runbook.md` has been
  recorded for at least one `v0.8.x` tag.
- [ ] `docs/v1-contract.md` has been reviewed against the current CLI, schemas,
  plugin protocol, run-directory artifacts, provider docs, package metadata,
  and Rust library exports.
- [ ] Every `stable-for-1.0` surface has tests, fixtures, schemas, or
  documentation evidence proportional to its compatibility risk.
- [ ] Every `pre-1.0-review-required` surface has either been promoted to
  stable, narrowed, marked experimental, or documented as internal.
- [ ] Machine-readable output changes are additive or covered by a new schema,
  format, or protocol version.
- [ ] Stable process exit-code meanings are unchanged or have a documented
  deprecation and migration path.
- [ ] Plugin and provider docs avoid unsupported claims about sandboxing,
  signing, certification, or live-provider reliability.
- [ ] Package-manager, signing, SBOM, and provenance claims match the artifacts
  actually produced for the release.
- [ ] `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `cargo test --workspace --locked --no-fail-fast`, schema checks,
  plugin fixture checks, governance checks, ecosystem checks, real-world
  workflow checks, and event-streaming smoke checks pass.

Release tags must be created from a branch that has passed required CI, or the
tag workflow must run an equivalent preflight before publishing artifacts. The
release artifact workflow must not be the first place v1.0 clippy, tests,
schema, plugin, governance, or ecosystem gates run.
