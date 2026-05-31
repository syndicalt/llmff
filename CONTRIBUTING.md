# Contributing

Contributions should preserve llmff's public contracts and keep examples
copy-pasteable. Prefer small changes with focused tests, fixture updates, and
documentation for user-visible behavior.

## Local Checks

Run the narrowest check that proves the change, then run broader checks before
opening a release-facing pull request.

Useful gates:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test
python3 scripts/check-schema-contract.py
bash scripts/check-plugin-fixtures.sh
bash scripts/check-governance-readiness.sh
```

Release-facing pull requests should also run the CI-equivalent local gate:

```bash
cargo test --workspace --locked --no-fail-fast
bash scripts/check-ecosystem-readiness.sh
bash scripts/check-real-world-workflows.sh
LLMFF_BIN=target/debug/llmff bash scripts/smoke-events-streaming.sh
```

Live provider smoke checks remain explicit and opt-in; do not require provider
secrets for ordinary pull requests.

For release packaging or distribution docs, also run the relevant shell gate
from `scripts/`.

## Stages

Stages are pipeline operations exposed through manifests or inline graphs. When
adding or changing a stage:

- define the manifest shape and validation errors before implementation;
- add deterministic tests that do not require remote providers;
- update stage listing metadata and examples when the stage is user-facing;
- keep trace/event output additive and consistent with
  `docs/governance.md`;
- document any compatibility impact in the release notes or readiness docs.

Stage implementations should keep inputs and outputs explicit. Avoid hidden
global state, mutable working-directory assumptions, and provider calls in tests
unless the test is explicitly a live-provider smoke test.

## Plugins

Plugins are local executables declared by `llmff-plugin.yaml`. When adding
plugin capabilities, examples, or validation behavior:

- preserve plugin protocol `1` compatibility unless a new protocol version is
  intentionally introduced;
- update `docs/plugins.md`, protocol fixtures, registry examples, and trust
  guidance together;
- validate example plugins with `scripts/check-plugin-fixtures.sh`;
- keep stdout reserved for protocol payloads and stderr for diagnostics;
- avoid registry entries that imply remote trust, signing, or sandboxing before
  those controls exist.

Breaking protocol changes need a governance review, migration notes, and
fixtures for both the old and new behavior during the deprecation window.

## Providers

Providers connect llmff to model backends and OpenAI-compatible services. When
adding or changing a provider:

- include a deterministic mock or fixture path for tests;
- document required environment variables, endpoint assumptions, and known
  limitations;
- keep provider configuration names stable once documented;
- avoid leaking prompts, secrets, provider payloads, or raw response bodies in
  stable failure events;
- mark live-provider smoke checks separately from default local tests.

Provider examples should be safe templates. Do not commit real credentials,
account-specific endpoints, or organization secrets.

## Documentation and Compatibility

User-facing docs should say whether a feature is supported, experimental,
prototype-only, or parked. Package-manager metadata can be prepared before a
channel is published, but docs must not imply support until maintainers decide
the channel is support-ready.

Deprecations must follow `docs/governance.md`: document the replacement,
warning behavior when practical, release-note impact, and removal timing.

## Deprecating Public Surface

Before deprecating a public manifest field, CLI flag, plugin protocol behavior,
trace/event field, run-directory artifact, provider configuration key, package
metadata channel, or library API:

- identify whether the change is patch, minor, or major under
  `docs/governance.md`;
- write the replacement or explain why there is no replacement;
- add tests, fixtures, schemas, or docs for the old and new behavior during the
  deprecation window;
- add release-note coverage with first deprecated release and earliest removal
  timing;
- keep warnings or diagnostics actionable and avoid breaking automation unless
  the old behavior is unsafe.
