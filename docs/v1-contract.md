# llmff v1.0 Contract Map

This document classifies the public surfaces that must be reviewed before the
`v1.0` release. It is intentionally conservative: a surface is stable only
when it is documented, schema-backed or fixture-backed where practical, and
safe for downstream automation to depend on.

`llmff` remains a bounded execution runner for typed inference pipelines. The
v1.0 contract does not include autonomous planning, model serving, agent loops,
global memory, task scheduling, provider account policy, or plugin sandboxing.

## Classification Labels

- `stable-for-1.0`: intended to remain backward compatible through the `1.x`
  CLI line, subject to the governance and deprecation policies.
- `pre-1.0-review-required`: intended to be useful, but must be reviewed,
  tested, documented, or narrowed before it can be promised stable.
- `experimental`: usable with explicit caveats; may change before or after
  `v1.0` without the same compatibility promise as stable surfaces.
- `internal`: implementation detail; downstream users must not depend on it.

## Stable For 1.0

### Product Boundary

Stable promise:

- `llmff` executes declared manifests and inline graphs as a supervised
  subprocess.
- Callers own orchestration, task planning, credential policy, artifact
  retention, live-provider qualification, and retry decisions above the
  pipeline-run level.

Evidence:

- `SPEC.md`
- `README.md`
- `docs/agent-workflows.md`

### CLI Execution Surface

Stable commands and process contract:

- `llmff run <manifest>`
- `llmff run -g/--graph <inline-graph>`
- `llmff inspect <manifest>`
- `llmff inspect -g/--graph <inline-graph>`
- `llmff doctor`
- documented process exit-code meanings
- stdout, stderr, `--trace`, `--events`, `--stream-stage`, manifest output,
  and run-directory artifact ownership rules

Stable `run` options:

- `--input`
- `--graph`
- `--trace`
- `--events`
- `--run-dir`
- `--parallel`
- `--max-concurrency`
- `--timeout-ms`
- `--retry-attempts`
- `--retry-backoff-ms`
- `--checkpoint`
- `--resume`
- `--replay-trace`
- `--batch-input`
- `--batch-output-dir`
- `--plugin-dir`
- `--stream-stage`
- `--backend`
- `--ollama`
- `--api-key-env`
- `--api-key`

Stable `inspect` options:

- `--input`
- `--graph`
- `--trace`
- `--events`
- `--parallel`
- `--max-concurrency`
- `--timeout-ms`
- `--retry-attempts`
- `--retry-backoff-ms`
- `--checkpoint`
- `--resume`
- `--stream-stage`
- `--format text`
- `--format json`
- `--plugin-dir`
- `--backend`
- `--ollama`
- `--api-key-env`
- `--api-key`

Stable `doctor` options:

- `--run-dir`
- `--plugin-dir`
- `--backend`
- `--api-key-env`
- `--release-manifest`

Compatibility rule:

- Adding flags is minor-version compatible.
- Renaming, removing, or changing behavior of documented flags requires
  deprecation unless the current behavior is unsafe or can lose data.
- Human-readable wording may change. Machine-readable output must stay
  additive unless a new format or schema version is introduced.

Evidence:

- `crates/llmff-cli/src/commands.rs`
- `docs/compatibility/core-contract-v1.md`
- `docs/schemas/run-result-v1.schema.json`
- `fixtures/golden/run-results/`

### Discovery CLI Surface

Stable commands:

- `llmff stages list`
- `llmff stages list --format json`
- `llmff backends list`
- `llmff backends list --format json`
- `llmff backends report`
- `llmff backends report --format json`
- `llmff models list`
- `llmff models list --format json`
- `llmff plugins list --plugin-dir <path>`
- `llmff plugins list --plugin-dir <path> --format json`
- `llmff plugins validate --plugin-dir <path>`
- `llmff plugins validate --plugin-dir <path> --format json`
- `llmff trace <path>`

Stable machine-readable expectations:

- `plugins validate --format json` uses
  `plugin-validation-report-v1.schema.json`.
- `stages list --format json`, `backends list --format json`, `backends report
  --format json`, `models list --format json`, and `plugins list --format json`
  are fixture-backed discovery contracts. Existing record keys and value
  meanings stay stable; minor releases may add optional fields and records.

Evidence:

- `docs/plugins.md`
- `docs/providers/support-tiers.md`
- `docs/provider-smoke-readiness.md`
- `docs/schemas/plugin-validation-report-v1.schema.json`
- `fixtures/golden/discovery/`
- `crates/llmff-cli/tests/cli_run.rs`

### Manifest Schema And Inline Graph Syntax

Stable surfaces:

- manifest `version: 1`
- schema-backed manifest fields in
  `docs/schemas/pipeline-manifest-v1.schema.json`
- inline graph syntax version `1` as documented in the compatibility guide
- stage graph validation rules that protect references, cycles, stdout
  ownership, required fields, and conservative type compatibility

Compatibility rule:

- Optional manifest fields, new stage operations, and new enum values are
  additive only when existing valid manifests remain valid.
- Breaking manifest changes require a new manifest schema version or a
  deprecation path.
- Breaking inline graph syntax requires a new inline graph syntax version.

Evidence:

- `docs/schemas/pipeline-manifest-v1.schema.json`
- `docs/schemas/README.md`
- `docs/compatibility/core-contract-v1.md`
- `fixtures/golden/manifests/`

### Trace, Event, Inspect, And Run-Result Schemas

Stable schemas:

- `event-v1.schema.json`
- `trace-v1.schema.json`
- `inspect-report-v1.schema.json`
- `run-result-v1.schema.json`
- `failure-kinds-v1.json`

Stable expectations:

- JSONL records are append-only protocols.
- Consumers must ignore unknown fields.
- Producers may add optional fields and new event names in minor releases.
- Existing field names, types, and meanings require deprecation or a new schema
  version to break.
- New `failure_kind` values are additive only when they are added to the
  failure-kind list, schemas, fixtures, and compatibility docs together.
- Exit code `130` remains the authoritative interrupted-run signal.

Evidence:

- `docs/schemas/`
- `docs/compatibility/core-contract-v1.md`
- `fixtures/golden/events/`
- `fixtures/golden/traces/`
- `fixtures/golden/inspect/report.json`
- `fixtures/golden/run-results/`

### Plugin Protocol V1

Stable protocol:

- plugin manifest schema version `1`
- plugin protocol version `1`
- capability kinds: `backend`, `sampler`, `stage`, `tool-transport`
- relative entrypoint resolution from plugin root
- one process per capability call
- stdin/stdout framing for stage, tool transport, backend, and sampler
- structured plugin validation report version `1`

Compatibility rule:

- Additive JSON fields are compatible when plugins can ignore them.
- Changing framing, entrypoint resolution, capability kind meanings, required
  JSON fields, validation report shape, or process lifecycle requires a new
  plugin protocol version and migration notes.
- Plugin validation is not sandboxing and must not be described as sandboxing.

Evidence:

- `docs/plugins.md`
- `docs/plugins/fixtures/protocol-v1/`
- `docs/plugins/trust.md`
- `docs/schemas/plugin-manifest-v1.schema.json`
- `docs/schemas/plugin-validation-report-v1.schema.json`
- `fixtures/golden/plugin-validation/`
- `examples/plugins/`

### Built-In Stage Names And Core Semantics

Stable stage operation names:

- `load`
- `template`
- `system`
- `infer`
- `validate_json`
- `repair`
- `retrieve`
- `rerank`
- `cache`
- `route`
- `tool`
- `write`

Stable expectation:

- Existing stage names and documented manifest fields keep their meaning within
  the `1.x` line.
- `when` is a documented conditional field on stages, not a stage operation.
- New stages and optional stage fields may be added in minor releases.
- Changing deterministic stage output shape, route behavior, validation
  failure semantics, cache policy meanings, or write/artifact behavior requires
  deprecation unless the current behavior is unsafe.

Evidence:

- `crates/llmff-core/src/stage.rs`
- `crates/llmff-core/src/manifest.rs`
- `docs/pipeline-library.md`
- `examples/templates/`
- `examples/real-world/`

### Provider Registration And Built-In Backend Families

Stable surfaces:

- mock backend aliases used by tests and offline examples
- OpenAI-compatible backend registration through `--backend`
- Ollama backend registration through `--ollama`
- API-key configuration through `--api-key-env` and `--api-key`
- provider capability reporting fields for JSON mode, streaming, seed, stop
  sequences, usage metadata, authentication, and diagnostics

Compatibility rule:

- Backend registration names and provider report fields are CLI automation
  surfaces once documented.
- Live-provider behavior is not guaranteed by the stable contract unless the
  caller or maintainer has run the relevant opt-in live smoke gate for that
  provider and release.

Evidence:

- `docs/providers/`
- `docs/provider-troubleshooting.md`
- `docs/provider-smoke-readiness.md`
- `.github/workflows/live-provider-smoke.yml`
- `scripts/smoke-openai-compatible-provider.sh`
- `scripts/smoke-ollama-provider.sh`

### Distribution Baseline

Stable distribution lane:

- GitHub Release assets and checksums are the default supported release lane.
- Cargo source install remains a supported fallback for source users.
- Release trust manifest documents checksum-only posture where stronger
  signing, SBOM, or provenance artifacts are not available.

Stable caveats:

- Windows MSI and macOS pkg artifacts are unsigned until signing credentials
  and recovery procedures exist.
- Homebrew, Scoop, winget, and AUR metadata are support-ready only after
  maintainers decide to publish each channel.
- Apt remains parked until signed repository metadata, hosting, key rotation,
  retention, and recovery are designed.

Evidence:

- `README.md`
- `docs/platform-support.md`
- `docs/distribution-trust.md`
- `docs/release-readiness.md`
- `.github/workflows/release-artifacts.yml`

### Rust Library API

The Rust crate exposes a stable-for-1.0 library API through public modules in
`llmff-core`, including:

- `Engine`
- `RunOptions`
- `RunReport`
- `RunStatus`
- `SchedulerMode`
- `RetryPolicy`
- `Backend`
- `InferRequest`
- `InferResponse`
- `InferStreamChunk`
- `UsageMetadata`
- `Manifest`
- `InputSpec`
- `OutputSpec`
- `StageSpec`
- `RetrySpec`
- `Graph`
- `Value`
- `Message`
- `StageStatus`
- plugin discovery and validation structs/functions
- `TraceEvent`
- `TraceWriter`
- `execute_deterministic_stage`

Compatibility rule:

- Public type and function names listed above remain backward compatible within
  the `1.x` line.
- Adding exported items is minor-version compatible.
- Changing public signatures, removing exports, changing documented error
  categories, or changing serialization fields on exported structs requires
  deprecation or a new major version.
- Private submodules under `engine/` and `stage/` remain internal even though
  they support public modules.

Evidence:

- `crates/llmff-core/src/lib.rs`
- `crates/llmff-core/src/*.rs`

## V1 Review Decisions

### Internal Module Boundaries

Decision: resolved for v1.0. The large engine, CLI command, and deterministic
stage internals have been split behind facade modules while preserving CLI
behavior and schema compatibility. Exact private helper layout remains internal
and is not part of the stable contract.

Evidence:

- `docs/superpowers/plans/2026-05-30-llmff-v1-roadmap.md`
- `crates/llmff-core/src/engine/`
- `crates/llmff-core/src/stage/`
- `crates/llmff-cli/src/commands/`

### Checkpoint, Replay, Batch, And Parallel Semantics

Decision: stable at the supervisor contract boundary and internal inside
payload details. Production supervisors may rely on:

- checkpoint reuse rules and manifest hash mismatch diagnostics
- replay trace constraints
- batch item isolation and batch report shape
- `--parallel` scheduling behavior and `--max-concurrency`
- restrictions between batch mode, explicit trace/event/checkpoint flags, and
  streaming flags
- run-dir artifact paths, process exit code preservation, and safe failure
  kinds

Checkpoint and batch checkpoint payload internals remain implementation
details. Callers should use `--resume`, manifest hashes, exit codes,
`result.json`, events, traces, and batch reports rather than parsing checkpoint
internals.

Evidence:

- `docs/agent-harness-contract.md`
- `docs/execution.md`
- `docs/agent-workflows.md`
- `crates/llmff-cli/tests/cli_run.rs`
- `crates/llmff-core/src/engine.rs`

### Discovery JSON

Decision: promoted to stable fixture-backed discovery contracts. The discovery
outputs listed in the stable discovery CLI section have representative golden
fixtures under `fixtures/golden/discovery/` and are checked by
`scripts/check-schema-contract.py`.

### Provider Support Tiers

Decision: resolved as evidence-backed support tiers. Provider pages and
`docs/providers/live-smoke-history.json` classify providers as documented only,
mock-inspectable, opt-in smoke ready, or live-smoke verified. No provider is live-smoke verified until a real smoke result is recorded.

Evidence:

- `docs/providers/support-tiers.md`
- `docs/providers/live-smoke-history.json`
- `docs/provider-smoke-readiness.md`
- `.github/workflows/live-provider-smoke.yml`

### Package-Manager Channels

Decision: metadata may remain checked in, but channel publication is outside
the v1.0 stable promise until maintainers choose to publish and support each
channel. GitHub Release assets plus Cargo source install remain the stable
distribution baseline.

Evidence:

- `docs/package-manager-roadmap.md`
- `packaging/`
- `docs/release-readiness.md`

## Experimental

Experimental surfaces may be useful, but callers should not treat them as
stable production commitments without a release note that promotes them.

- Optional OpenTelemetry bridge design and related local exporter hooks.
- Static plugin registry promotion beyond official examples.
- Package-manager publication metadata before maintainer support approval.
- Live provider smoke workflows for provider drift detection.
- Future plugin signing guidance.
- Future SBOM/provenance artifacts beyond the current release trust manifest.

Evidence:

- `docs/opentelemetry-bridge.md`
- `docs/plugins/registry.md`
- `docs/plugins/promotion-policy.md`
- `docs/plugins/trust.md`
- `docs/package-manager-roadmap.md`
- `docs/distribution-trust.md`
- `docs/provider-smoke-readiness.md`

## Internal

The following are implementation details and must not be used as external
contracts:

- private helper functions in `engine.rs`, `stage.rs`, `commands.rs`, and
  `plugin.rs`
- exact Rust module layout before the v1.0 API freeze
- clippy-satisfying context structs introduced during refactors
- exact wording of human-readable diagnostics, except where release notes
  document a compatibility-sensitive diagnostic
- temporary files and directories created by tests or smoke scripts
- generated package build intermediates such as WiX payload roots or macOS
  package roots
- local-only roadmap planning files under `docs/superpowers/`

## Required Bump Rules After V1.0

Major version or new contract version required:

- removing or renaming documented CLI flags or subcommands
- changing the meaning of documented CLI automation surfaces
- breaking existing valid `version: 1` manifests
- changing inline graph syntax incompatibly without a new inline syntax version
- changing trace/event/inspect/run-result field types or meanings
- changing plugin protocol framing, capability kind meanings, or process
  lifecycle semantics
- changing documented exit-code meanings
- changing stable run-dir artifact semantics relied on by supervisors

Minor version compatible:

- adding optional manifest fields
- adding stage operations without changing existing stage behavior
- adding optional JSON fields to machine-readable output
- adding new `failure_kind` values with schema, fixture, and docs updates
- adding provider examples or support-tier evidence
- adding plugin capability metadata fields that protocol v1 plugins can ignore

Patch version compatible:

- bug fixes that preserve documented behavior
- clearer human-readable diagnostics
- documentation corrections
- test, fixture, and packaging-script fixes that do not change public
  contracts

## V1.0 Audit Checklist

Before `v1.0.0`, maintainers must verify:

- every `stable-for-1.0` surface above has tests, fixtures, schemas, or
  documented evidence proportional to its risk
- every `pre-1.0-review-required` surface has either been promoted, narrowed,
  or explicitly left out of the stable promise
- release notes describe any pre-1.0 behavior that changed during the freeze
- package-manager and signing claims do not exceed implemented support
- plugin and provider docs do not imply sandboxing, certification, or live
  support without evidence
- the full release gate in `docs/release-readiness.md` passes
