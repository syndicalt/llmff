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
- [x] Release tag workflows enforce signing and notarization release gates before macOS or Windows installer publication.

Do not describe `llmff` as broadly released until every item in this section is checked.

Release `v0.1.1` was verified with:

```bash
scripts/release-preflight.sh v0.1.1
scripts/smoke-install.sh --git https://github.com/syndicalt/llmff --tag v0.1.1
```

For release tags after this packaging slice, CI creates the GitHub Release when the tag does not already have one, then uploads binary archives, checksums, Ubuntu/Debian packages, and Arch packaging metadata to the matching GitHub Release assets. Manual dispatch keeps those outputs as Actions artifacts only.

Release-tag CI also runs signing and notarization release gates before native
macOS or Windows installer publication. Windows release tags sign and verify
the MSI with Authenticode before regenerating its checksum and smoke-testing
the signed package. macOS release tags sign, notarize, staple, and smoke-test
the `.pkg` before upload. Manual workflow dispatch remains available for
unsigned artifact testing.

Current packaged artifact targets and installer assumptions are documented in
[`docs/platform-support.md`](platform-support.md).
