# Plugin Trust Model

Plugins are local executables. llmff validates manifests and entrypoint paths,
but it does not sandbox plugin code.

## Permissions

A plugin runs with the same operating-system permissions as the `llmff` process.
It can read files the user can read, write files the user can write, spawn child
processes, and open network connections if the host allows it.

Plugin authors should document every expected file, process, environment, and
network access. Plugin users should treat undocumented access as a review issue.

## Sandbox Expectations

Protocol version `1` does not provide a built-in sandbox. Use OS-level controls
when running untrusted or semi-trusted plugins:

- Run in a dedicated user account or container.
- Mount only the directories the plugin needs.
- Pass explicit environment variables instead of inheriting broad secrets.
- Disable network access unless the plugin requires it.
- Keep plugin dependencies pinned and reproducible.

## Review Checklist

- Manifest name, version, capability kind, capability name, and entrypoint are
  intentional.
- Entrypoints are repository-local scripts or pinned binaries, not mutable global
  commands.
- Stdout is reserved for protocol output; logs and diagnostics go to stderr.
- JSON-producing plugins emit one complete JSON document.
- File, environment, process, and network access are documented.
- Dependencies are pinned and can be rebuilt or audited.
- Error paths fail non-zero and include actionable stderr.
- CI runs `llmff plugins validate` and protocol fixture checks.

## Registry Promotion Review

Promoted registry entries carry a stronger maintenance promise than unreviewed
metadata. The registry must point at `docs/plugins/promotion-policy.md`, and
each promoted entry must have a matching JSON review under `docs/plugins/reviews/`.

The review records the protocol fixtures that cover the plugin category and the
expected trust boundary. Any registry change that changes the manifest,
capability, support commitment, or trust boundary needs a fresh review record.

## Optional Plugin Signing

Plugin signing is separate from application release signing. A future signing
layer should sign plugin manifests and immutable entrypoint artifacts, publish
the public verification key, and keep trust decisions explicit at install or run
time.

Signing is not part of plugin protocol `1`, and current validation does not
verify signatures.
