# Plugin Registry Promotion Policy

Promotion is a support commitment, not generated metadata. A promoted registry
entry means llmff maintainers intend to keep that plugin compatible with plugin
protocol version `1` until the protocol compatibility policy changes.

## Promoted Registry Entries

Promoted registry entries must have:

- a local or immutable manifest reference;
- protocol fixture coverage for the plugin category;
- review evidence in `docs/plugins/reviews/`;
- an explicit trust-boundary review;
- a `protocol-v1-fixture-backed` support commitment in the registry and review
  record.

The static registry is still not an installer. Runtime loading continues to use
`--plugin-dir` and local plugin manifests. Promotion only means the published
registry entry is maintained as a documented, fixture-backed example. Promotion
does not imply plugin signing, sandboxing, remote trust, or provider
certification.

## Review Evidence

Each promoted plugin must have one JSON review record. The record repeats the
registry promotion and trust-boundary fields so CI can compare them directly.
The record also names the validation command and protocol fixture files that
cover the promoted category.

## Trust Boundary

Protocol version `1` plugins are unsandboxed local executables. A trust-boundary
review must document expected filesystem, network, process, and environment
access. Any new access beyond the reviewed boundary requires a fresh review
before promotion.
