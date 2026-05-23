# Governance

This project treats manifests, plugin protocols, CLI automation surfaces, and
trace/event output as public contracts once they are documented for users or
integrators. Changes to those contracts should be deliberate, reviewable, and
recoverable.

## Decision Owners

Maintainers decide when a distribution channel, contract version, or ecosystem
integration is support-ready. Prototype metadata and local validation scripts
may exist before that decision, but they do not imply that a channel is
published or supported.

For compatibility-affecting changes, maintainers should identify:

- the contract being changed;
- whether the change is additive, behavioral, or breaking;
- the release where the change first appears;
- the migration path for existing users;
- the tests, fixtures, or docs that prove the intended behavior.

## Stability Policy

### Manifest schema stability

Versioned pipeline manifest fields are stable once documented in
`docs/schemas/` or used by checked-in examples. Minor releases may add optional
fields, new stage operations, new enum values, or stricter diagnostics that do
not reject previously valid manifests. Removing fields, changing field
meanings, or rejecting a previously valid manifest for reasons other than
security or data-loss risk requires a deprecation period or a new schema
version.

### Plugin protocol stability

Plugin protocol `1` is backward compatible within the current major CLI line.
Additive request or response fields are allowed when plugins can ignore them.
Changing stdin/stdout framing, entrypoint resolution, capability kind meanings,
required JSON fields, validation report shape, or process lifecycle semantics
requires a new plugin protocol version and migration notes.

### CLI flag stability

Documented CLI flags, subcommands, exit-code expectations, and machine-readable
output formats are automation surfaces. Additive flags and subcommands are
allowed in minor releases. Renaming or removing a documented flag, changing the
meaning of an existing flag, or changing machine-readable output in a way that
breaks parsers requires deprecation unless the existing behavior is unsafe.

Human-readable wording may change without deprecation. Machine-readable output
should prefer stable field names, additive fields, and explicit format versions.

### Trace and event field stability

Trace and event JSONL are append-only protocols. Existing event names, required
fields, and field meanings stay stable within a major version. Consumers must
ignore unknown fields, and producers may add optional fields or new event names
in minor releases. Removing fields, changing field types, or changing the
meaning of existing trace/event fields requires deprecation or a new schema
version.

## Deprecation Policy

Deprecation policy applies to manifest fields, plugin protocol behavior,
documented CLI flags, machine-readable CLI output, trace/event fields, package
metadata channels, and provider configuration keys.

Deprecations must include:

- a replacement or migration path;
- a release note entry;
- a warning or diagnostic when the deprecated surface is used, when practical;
- compatibility fixtures or tests that cover the old and new behavior during
  the deprecation window;
- removal timing expressed as a minimum release or major-version boundary.

Default deprecation window:

- Public schemas, plugin protocol behavior, CLI automation surfaces, and
  trace/event fields: keep for the rest of the current major version unless the
  behavior is unsafe or unrecoverable.
- Experimental examples, provider presets, or package-manager prototype
  metadata: keep for at least one minor release after replacement is documented.
- Security, credential, or data-loss risks: maintainers may remove or disable
  immediately, but the release notes must explain the reason and recovery path.

Deprecated behavior must not be silently repurposed. If a name is removed, do
not reuse it for different semantics until a new major version or contract
version makes the break explicit.

## Release Governance

Before a release is promoted beyond GitHub Release assets, maintainers must run
the documented readiness gates and review the ecosystem compatibility checklist
in `docs/release-readiness.md`.

The governance readiness gate is:

```bash
scripts/check-governance-readiness.sh
```

That check is intentionally text-based. It protects policy artifacts that are
easy to drop during parallel roadmap work; it does not replace contract tests,
schema validation, package smoke tests, or maintainer review.
