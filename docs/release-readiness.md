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

- [ ] A versioned tag or release exists.
- [ ] Fresh install has been verified from that tag or release.
- [ ] Release notes describe supported platforms and known limitations.
- [ ] The smoke install gate passes against the release source, not only the local checkout.
- [ ] At least one end-to-end example can be run by a new user without editing repository files.

Do not describe `llmff` as broadly released until every item in this section is checked.

Use the tag-specific smoke gate for release verification:

```bash
scripts/smoke-install.sh --git https://github.com/syndicalt/llmff --tag v0.1.0
```
