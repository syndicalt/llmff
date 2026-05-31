# Release Runbook

This runbook is the operational checklist for the v1.0 release-candidate train.
It separates local release preparation from externally observable release
evidence so the roadmap is not marked complete before tags, CI, and published
assets exist.

## Evidence Classes

- Local preparation: checks that can pass before a tag exists. These prove the
  source tree is ready to tag, but they do not prove installability from GitHub
  or published artifact availability.
- Published release candidate: evidence collected after a tag has been pushed
  and release-tag CI has completed.
- Final v1.0 release: evidence collected after at least one release candidate
  has validated the stable contract and the final compatibility review is
  complete.

## V0.8.0 Release Candidate

Do not mark the roadmap release-candidate step complete until all of this
evidence exists for the same commit and tag.

1. Local preflight passes before tagging:

   ```bash
   cargo fmt --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace --locked --no-fail-fast
   python3 scripts/check-schema-contract.py
   bash scripts/check-plugin-fixtures.sh
   bash scripts/check-governance-readiness.sh
   bash scripts/check-ecosystem-readiness.sh
   bash scripts/check-real-world-workflows.sh
   LLMFF_BIN=target/debug/llmff bash scripts/smoke-events-streaming.sh
   scripts/release-preflight.sh v0.8.0
   ```

2. Create an annotated tag from the reviewed release commit:

   ```bash
   git tag -a v0.8.0 -m "llmff v0.8.0"
   git push origin v0.8.0
   ```

3. Wait for the release-tag artifact workflow to complete successfully for
   `v0.8.0`.

4. Verify the published GitHub Release assets:

   ```bash
   scripts/check-release-assets.sh v0.8.0
   ```

5. Verify installability from the published tag:

   ```bash
   scripts/smoke-install.sh --git https://github.com/syndicalt/llmff --tag v0.8.0
   ```

6. Record the release-candidate outcome in the release issue, pull request, or
   release notes. The record must include the commit SHA, tag, CI run URL,
   `scripts/check-release-assets.sh v0.8.0` result, and
   `scripts/smoke-install.sh --git ... --tag v0.8.0` result.

Checked-in release-candidate evidence should live under
`docs/release-evidence/` when a release-candidate roadmap step is marked
complete.

## V1.0.0 Final Release

Do not publish `v1.0.0` until the release-candidate evidence above exists and
the v1.0 compatibility review is complete.

Required final evidence:

- required local and CI gates pass on the release branch;
- `docs/v1-contract.md` has no `pre-1.0-review-required` surfaces;
- public machine-readable outputs have schema or fixture coverage;
- package artifacts build and smoke-test on their target platforms;
- dependency and security review is recorded in the release issue or notes;
- dependency and security review evidence is recorded before the final tag;
- `docs/migration/pre-1.0-to-1.0.md` is complete;
- unsigned Windows and macOS status is repeated unless signing and notarization
  are live;
- package-manager publication claims match channels that maintainers have
  explicitly marked support-ready.

After the final tag is pushed and release-tag CI completes, verify:

```bash
scripts/check-release-assets.sh v1.0.0
scripts/smoke-install.sh --git https://github.com/syndicalt/llmff --tag v1.0.0
```

Only then mark the roadmap v1.0 shipping step complete.
