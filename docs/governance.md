# Governance

This project treats manifests, plugin protocols, CLI automation surfaces, and
trace/event output as public contracts once they are documented for users or
integrators. Changes to those contracts should be deliberate, reviewable, and
recoverable.

`docs/v1-contract.md` is the release contract map for `v1.0`. It classifies
surfaces as stable, pre-1.0 review required, experimental, or internal. Before
`v1.0.0`, every stable surface in that map must have evidence proportional to
its risk, and every review-required surface must be promoted, narrowed, or left
out of the stable promise.

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
in minor releases when schemas, fixtures, and compatibility docs are updated
together. Removing fields, changing field types, or changing the meaning of
existing trace/event fields requires deprecation or a new schema version.

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

## Post-v1 Semver Examples

Use these examples after `v1.0.0` when deciding whether a change is patch,
minor, or major.

### Manifest schema semver examples

- Patch: clarify validation diagnostics, fix a parser bug so documented valid
  manifests work, or add missing schema docs without changing accepted input.
- Minor: add an optional manifest field, new stage operation, new enum value,
  or new validation warning that does not reject previously valid manifests.
- Major: remove or rename a documented field, change a field's meaning, or
  reject previously valid manifests outside a security or data-loss fix.

### CLI flag semver examples

- Patch: fix help text, exit-code mapping for an already documented failure, or
  a flag parser bug that prevented documented usage.
- Minor: add a new subcommand, flag, output format, or optional JSON field.
- Major: remove or rename a documented flag, change a flag's behavior in a way
  that breaks scripts, or alter stable exit-code meanings.

### Plugin protocol semver examples

- Patch: fix validation diagnostics or command execution bugs while preserving
  protocol version `1` framing and JSON contracts.
- Minor: add optional request or response fields that plugins can ignore, or
  add a new capability kind behind a documented protocol extension.
- Major: change stdin/stdout framing, required backend or sampler JSON fields,
  entrypoint resolution, process lifecycle, or validation report semantics
  without a compatible protocol version.

### Trace and event schema semver examples

- Patch: correct missing safe metadata, fix timestamps or duration units to
  match docs, or restore an omitted required field.
- Minor: add optional event fields, new event names, or new failure kinds with
  schema and fixture coverage.
- Major: remove fields, change field types, change event meanings, or make
  consumers reinterpret existing `failure_kind` values.

### Library API semver examples

- Patch: fix a bug in an exported function while preserving signatures,
  behavior contracts, and error categories.
- Minor: add a new exported type, function, trait method with a default
  implementation, or non-exhaustive enum variant.
- Major: remove or rename exported items, change public signatures, make
  existing enum matches fail without a non-exhaustive contract, or change error
  semantics relied on by library users.

## Deprecation Checklist Template

Use this checklist before deprecating any public surface:

- Name the public surface and owning contract: manifest schema, CLI flag,
  plugin protocol, trace/event schema, run-directory artifact, provider
  configuration, package metadata, or library API.
- Classify the replacement as already available, newly added, or intentionally
  absent.
- Document the migration path and include at least one before/after example
  when the surface is user-authored.
- Add or update tests, fixtures, schemas, or docs that prove both the old
  behavior and replacement behavior during the deprecation window.
- Add a release-note entry that names the first deprecated release and the
  earliest removal release or major-version boundary.
- Emit a warning or diagnostic when practical without breaking automation.
- Confirm the old name will not be reused for different semantics before the
  next major version or explicit contract-version break.

## Release Governance

Before a release is promoted beyond GitHub Release assets, maintainers must run
the documented readiness gates and review the ecosystem compatibility checklist
in `docs/release-readiness.md`.

Before a `v1.0.0` release, maintainers must also review `docs/v1-contract.md`
and confirm that CLI automation surfaces, schemas, plugin protocol behavior,
trace/event fields, run-directory artifacts, and any stable Rust library API
are intentionally included in or excluded from the stable promise.

The governance readiness gate is:

```bash
scripts/check-governance-readiness.sh
```

That check is intentionally text-based. It protects policy artifacts that are
easy to drop during parallel roadmap work; it does not replace contract tests,
schema validation, package smoke tests, or maintainer review.
