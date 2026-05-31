# LLMFF V1 Roadmap Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move `llmff` from the current `v0.1.6` foundation to a production-ready `v1.0` release with stable contracts, strong validation gates, maintainable internals, and clear adoption paths.

**Architecture:** Treat `llmff` as a bounded FFmpeg-style execution runner, not an agent framework, model server, or orchestration platform. The v1.0 path narrows and proves the existing surface before adding new capability: contract audit, maintainability refactor, CI gates, diagnostics, documentation, plugin/provider support policy, then release candidates.

**Tech Stack:** Rust workspace with `llmff-core` and `llmff-cli`, Cargo tests and clippy, shell validation gates, JSON Schema compatibility fixtures, GitHub Actions release and provider-smoke workflows.

---

## Current Evidence

Repository audit date: 2026-05-30.

Passing checks:

- `cargo test --workspace --no-fail-fast`
- `cargo fmt --check`
- `python3 scripts/check-schema-contract.py`
- `scripts/check-plugin-fixtures.sh`
- `scripts/check-governance-readiness.sh`
- `scripts/check-ecosystem-readiness.sh`
- `scripts/check-real-world-workflows.sh`
- `LLMFF_BIN=target/debug/llmff bash scripts/smoke-events-streaming.sh`

Known failing gate:

- `cargo clippy --workspace --all-targets -- -D warnings` was failing at
  audit time; it now passes after the first v0.2 hardening slice.

Primary clippy blockers:

- Fixed: `crates/llmff-core/src/engine.rs` internal scheduler/stage execution
  calls now use internal context structs instead of long argument lists.
- Fixed: `crates/llmff-core/src/plugin.rs` private plugin-report parsing now
  boxes the large diagnostic error variant without changing public plugin
  structs.
- Fixed: `crates/llmff-cli/src/commands.rs` internal run, inspect, and batch
  helpers now use option structs instead of clippy-blocking long signatures.

Primary maintainability risks:

- `crates/llmff-core/src/engine.rs`: 5,749 lines.
- `crates/llmff-cli/src/commands.rs`: 2,382 lines.
- `crates/llmff-core/src/stage.rs`: 1,982 lines.
- `crates/llmff-cli/tests/cli_run.rs`: 5,015 lines.

Important scope boundary:

- Preserve `llmff` as a bounded execution runner for typed inference pipelines.
- Do not grow autonomous planning, task scheduling, memory, agent loops, provider account policy, or model serving into core.

## Version Track

- `v0.2`: public contract inventory, maintainability refactor, clippy gate.
- `v0.3`: production CI, failure diagnostics, run-dir/batch/streaming hardening.
- `v0.4`: adoption docs, cookbook routing, examples and supervisor guidance.
- `v0.5`: plugin/provider support policy and compatibility harness.
- `v0.8`: API freeze and first release candidate.
- `v1.0`: production contract release.

## Task 1: V1 Contract Audit And Scope Freeze

**Files:**

- Create: `docs/v1-contract.md`
- Modify: `docs/compatibility/core-contract-v1.md`
- Modify: `docs/governance.md`
- Modify: `docs/release-readiness.md`
- Test: `scripts/check-governance-readiness.sh`
- Test: `python3 scripts/check-schema-contract.py`

- [x] **Step 1: Inventory all public surfaces**

  Catalog the current stable, experimental, and internal surfaces:

  - CLI commands and flags from `crates/llmff-cli/src/commands.rs`.
  - Manifest schema and inline graph syntax from `docs/schemas/` and `docs/compatibility/core-contract-v1.md`.
  - Trace, event, inspect, plugin validation, and run-result schemas.
  - Plugin protocol v1 behavior and capability kinds.
  - Public Rust items exported by `llmff-core`.
  - Release artifacts and package-manager metadata.

- [x] **Step 2: Write `docs/v1-contract.md`**

  The document must classify each surface as one of:

  - `stable-for-1.0`
  - `pre-1.0-review-required`
  - `experimental`
  - `internal`

  It must also state what requires a major version, schema version, or plugin protocol version bump after `v1.0`.

- [x] **Step 3: Update governance and readiness docs**

  Link `docs/v1-contract.md` from governance and release readiness docs. Make v1.0 release readiness require a successful contract audit.

- [x] **Step 4: Validate**

  Run:

  ```bash
  scripts/check-governance-readiness.sh
  python3 scripts/check-schema-contract.py
  ```

  Expected: both pass.

## Task 2: Split Core Engine Internals Before API Freeze

**Files:**

- Modify: `crates/llmff-core/src/engine.rs`
- Create: `crates/llmff-core/src/engine/checkpoint.rs`
- Create: `crates/llmff-core/src/engine/execution.rs`
- Create: `crates/llmff-core/src/engine/scheduler.rs`
- Create: `crates/llmff-core/src/engine/streaming.rs`
- Create: `crates/llmff-core/src/engine/trace_failure.rs`
- Modify: `crates/llmff-core/src/lib.rs`
- Test: `crates/llmff-core/src/engine.rs` existing tests, moved or grouped as needed.

- [x] **Step 1: Introduce internal context structs**

  Replace long internal function signatures with focused structs:

  - `ExecutionContext`
  - `PluginExecutionContext`
  - `CheckpointContext`
  - `StageExecutionContext`
  - `SchedulerContext`

  Keep public `Engine`, `RunOptions`, `RunReport`, `RunStatus`, and `SchedulerMode` behavior unchanged.

- [x] **Step 2: Extract checkpoint and replay helpers**

  Move checkpoint read/write, manifest hash validation, and replay-trace validation into `engine/checkpoint.rs`.

- [x] **Step 3: Extract streaming ownership helpers**

  Move stage stream writer creation, stdout/file ownership checks owned by core, and selected-stage payload streaming into `engine/streaming.rs`.

- [x] **Step 4: Extract scheduler implementations**

  Move sequential and parallel scheduler logic into `engine/scheduler.rs`.

- [x] **Step 5: Extract trace and failure helpers**

  Move trace writer creation, `run_failed` emission, and core failure-kind mapping into `engine/trace_failure.rs`.

- [x] **Step 6: Validate**

  Run:

  ```bash
  cargo fmt --check
  cargo test -p llmff-core
  cargo clippy -p llmff-core --all-targets -- -D warnings
  ```

  Expected: all pass.

## Task 3: Split CLI Command Internals

**Files:**

- Modify: `crates/llmff-cli/src/commands.rs`
- Create: `crates/llmff-cli/src/commands/run.rs`
- Create: `crates/llmff-cli/src/commands/inspect.rs`
- Create: `crates/llmff-cli/src/commands/providers.rs`
- Create: `crates/llmff-cli/src/commands/plugins.rs`
- Create: `crates/llmff-cli/src/commands/run_dir.rs`
- Create: `crates/llmff-cli/src/commands/batch.rs`
- Create: `crates/llmff-cli/src/commands/exit_codes.rs`
- Modify: `crates/llmff-cli/src/main.rs`
- Test: `crates/llmff-cli/tests/cli_run.rs`

- [x] **Step 1: Preserve the existing clap surface**

  Keep current subcommands and flags behavior-compatible. This task is an internal module split, not a CLI redesign.

- [x] **Step 2: Move run-dir and batch helpers first**

  Extract run-dir artifacts, result summaries, interrupted-run helpers, batch execution, and batch failure mapping.

- [x] **Step 3: Move provider and plugin commands**

  Extract backend/model report code and plugin list/validate code into focused modules.

- [x] **Step 4: Move exit-code classification**

  Centralize process exit-code mapping and keep tests that prove supervisor-facing codes are stable.

- [x] **Step 5: Validate**

  Run:

  ```bash
  cargo fmt --check
  cargo test -p llmff --test cli_run
  cargo clippy -p llmff --all-targets -- -D warnings
  ```

  Expected: all pass.

## Task 4: Split Stage Implementations

**Files:**

- Modify: `crates/llmff-core/src/stage.rs`
- Create: `crates/llmff-core/src/stage/retrieval.rs`
- Create: `crates/llmff-core/src/stage/template.rs`
- Create: `crates/llmff-core/src/stage/validate.rs`
- Modify: `crates/llmff-core/src/lib.rs`
- Test: existing `stage::tests`.

- [x] **Step 1: Preserve `execute_deterministic_stage` as the public entry point**

  Keep the stage module API stable while moving implementation details behind submodules.

- [x] **Step 2: Move stage families one at a time**

  Move and test in this order: template/system, validate, retrieval/rerank.
  Keep cache, tool, and write in engine-owned code unless a later design
  proves moving them improves ownership without changing behavior.

- [x] **Step 3: Validate**

  Run:

  ```bash
  cargo fmt --check
  cargo test -p llmff-core stage::
  cargo clippy -p llmff-core --all-targets -- -D warnings
  ```

  Expected: all pass.

## Task 5: Make Clippy A Required Local And CI Gate

**Files:**

- Create or modify: `.github/workflows/ci.yml`
- Modify: `CONTRIBUTING.md`
- Modify: `docs/release-readiness.md`

- [x] **Step 1: Add PR CI**

  Add a standard CI workflow for pushes and pull requests:

  ```bash
  cargo fmt --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace --locked
  python3 scripts/check-schema-contract.py
  scripts/check-plugin-fixtures.sh
  scripts/check-governance-readiness.sh
  scripts/check-ecosystem-readiness.sh
  scripts/check-real-world-workflows.sh
  LLMFF_BIN=target/debug/llmff bash scripts/smoke-events-streaming.sh
  ```

- [x] **Step 2: Document the required local gate**

  Update `CONTRIBUTING.md` so release-facing PRs run the same gate locally or explain platform-specific skips.

- [x] **Step 3: Validate**

  Run the full CI command list locally.

  Expected: all pass.

## Task 6: Diagnostics And Error UX Hardening

**Files:**

- Modify: `crates/llmff-core/src/error.rs`
- Modify: `crates/llmff-core/src/engine/trace_failure.rs`
- Modify: `crates/llmff-cli/src/commands/exit_codes.rs`
- Modify: `docs/schemas/failure-kinds-v1.json`
- Modify: `fixtures/golden/events/failure.jsonl`
- Modify: `fixtures/golden/run-results/stage-failure.json`
- Test: `crates/llmff-cli/tests/cli_run.rs`
- Test: `crates/llmff-core` error/failure tests.

- [x] **Step 1: Define the top failure-mode matrix**

  Cover manifest parse, missing input, invalid graph, unknown stage, missing backend, invalid plugin, timeout, HTTP/server error, tool non-zero, schema invalid, stdout ownership conflict, checkpoint mismatch, batch item failure, interrupted run.

- [x] **Step 2: Add golden stderr and JSON tests**

  For each failure mode, assert:

  - process exit code
  - stderr has safe actionable text
  - `run_failed.failure_kind` is stable when an event writer exists
  - run-dir `result.json` preserves the same kind and exit code where applicable

- [x] **Step 3: Improve messages without changing machine contracts**

  Human-readable messages may change. Machine-readable names and field meanings must remain additive and schema-backed.

- [x] **Step 4: Validate**

  Run:

  ```bash
  cargo test --workspace --no-fail-fast
  python3 scripts/check-schema-contract.py
  ```

  Expected: all pass.

## Task 7: Add `llmff doctor` Only If It Checks Real Preconditions

**Files:**

- Modify: `crates/llmff-cli/src/commands.rs` or new command module from Task 3.
- Modify: `README.md`
- Modify: `docs/provider-troubleshooting.md`
- Test: `crates/llmff-cli/tests/cli_run.rs`

- [x] **Step 1: Decide whether `doctor` clears the usefulness bar**

  Include it only if it checks concrete local state:

  - binary version
  - writable run directory
  - plugin directory validation
  - provider alias env var presence without printing secrets
  - optional release asset trust manifest presence when installed from a release bundle

- [x] **Step 2: Keep it local and non-invasive**

  No network calls by default. Live provider checks remain explicit smoke scripts.

- [x] **Step 3: Validate**

  Run:

  ```bash
  cargo test -p llmff --test cli_run doctor
  ```

  Expected: doctor tests pass, or this task is closed as intentionally not implemented.

## Task 8: Adoption Documentation Pass

**Files:**

- Modify: `README.md`
- Modify: `SPEC.md`
- Modify: `docs/quickstart.md`
- Modify: `docs/agent-workflows.md`
- Modify: `docs/pipeline-library.md`
- Create: `docs/when-to-use-llmff.md`
- Create: `docs/cookbook.md`
- Create: `docs/migration/pre-1.0-to-1.0.md`
- Test: `crates/llmff-cli/tests/example_catalog.rs`
- Test: `scripts/check-agent-adoption-guide.sh`

- [x] **Step 1: Add decision guidance**

  Make the first-reader path clear:

  - use `llmff` for explicit typed inference sub-pipelines
  - do not use it as an agent framework, model server, scheduler, memory system, or autonomous planner

- [x] **Step 2: Route the cookbook to existing examples**

  Keep examples offline-runnable by default. Avoid duplicating CLI reference material.

- [x] **Step 3: Document the supervisor pattern as canonical**

  Preserve the subprocess pattern: inspect, run, preserve exit code, store run-dir artifacts, read safe failure kinds.

- [x] **Step 4: Validate**

  Run:

  ```bash
  cargo test -p llmff --test example_catalog
  scripts/check-agent-adoption-guide.sh
  scripts/check-real-world-workflows.sh
  ```

  Expected: all pass.

## Task 9: Plugin And Provider Support Lock

**Files:**

- Modify: `docs/plugins.md`
- Modify: `docs/plugins/registry.md`
- Modify: `docs/plugins/promotion-policy.md`
- Modify: `docs/providers/support-tiers.md`
- Modify: `docs/provider-smoke-readiness.md`
- Create: `examples/plugins/template/llmff-plugin.yaml`
- Create: `examples/plugins/template/README.md`
- Test: `scripts/check-plugin-fixtures.sh`
- Test: `scripts/check-provider-smoke-readiness.sh`
- Test: `crates/llmff-cli/tests/plugin_ecosystem.rs`

- [x] **Step 1: Publish a plugin template**

  Include minimal stage, backend, sampler, and tool-transport examples only if each is covered by validation.

- [x] **Step 2: Define support tiers**

  Tie provider support labels to evidence:

  - documented only
  - mock-inspectable
  - opt-in smoke ready
  - live-smoke verified

- [x] **Step 3: Keep trust language conservative**

  Preserve the no-sandboxing boundary. Do not imply plugin signing, remote trust, or provider certification until those controls exist.

- [x] **Step 4: Validate**

  Run:

  ```bash
  scripts/check-plugin-fixtures.sh
  scripts/check-provider-smoke-readiness.sh
  cargo test -p llmff --test plugin_ecosystem
  ```

  Expected: all pass.

## Task 10: V1.0 Release Candidate Train

**Files:**

- Modify: `docs/release-readiness.md`
- Create: `docs/release-notes/v0.8.0.md`
- Create: `docs/release-notes/v1.0.0.md`
- Create: `docs/release-runbook.md`
- Modify: `Cargo.toml`
- Modify: `crates/llmff-core/Cargo.toml`
- Modify: `crates/llmff-cli/Cargo.toml`
- Modify: package metadata under `packaging/`
- Test: release and package scripts.

- [x] **Step 1: Declare API freeze at `v0.8`**

  Stop adding new public surface unless it is required to fix a v1.0 blocker.

- [x] **Step 2: Run full release gate**

  Run:

  ```bash
  cargo fmt --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace --locked --no-fail-fast
  python3 scripts/check-schema-contract.py
  scripts/check-plugin-fixtures.sh
  scripts/check-governance-readiness.sh
  scripts/check-ecosystem-readiness.sh
  scripts/check-real-world-workflows.sh
  LLMFF_BIN=target/debug/llmff bash scripts/smoke-events-streaming.sh
  scripts/release-preflight.sh v0.8.0
  ```

- [ ] **Step 3: Cut at least one release candidate**

  Use the RC to validate install, artifacts, docs, examples, and external integration assumptions before `v1.0.0`.

  Local preparation completed: `v0.8.0` release preflight passes, Linux archive
  and Debian artifacts build and smoke-test locally, macOS payload roots
  smoke-test locally, Windows WiX source emits locally, Arch metadata is
  generated, and the release trust manifest is generated under
  `dist/v0.8.0-local/`. The actual release candidate tag, push, CI artifact
  matrix, and published GitHub Release assets remain external release actions.
  `docs/release-runbook.md` defines the evidence that must be recorded before
  this checkbox can be completed, including `scripts/check-release-assets.sh
  v0.8.0` and `scripts/smoke-install.sh --git
  https://github.com/syndicalt/llmff --tag v0.8.0` against the published tag.

- [ ] **Step 4: Ship `v1.0.0` only after compatibility review**

  Required evidence:

  - all local and CI gates pass
  - no undocumented public CLI flags
  - all schemas and fixtures reflect public machine-readable outputs
  - package artifacts build and smoke-test on their target platforms
  - dependency and security review completed
  - `docs/migration/pre-1.0-to-1.0.md` complete
  - `docs/release-runbook.md` final-release evidence recorded, including
    `scripts/check-release-assets.sh v1.0.0` and `scripts/smoke-install.sh
    --git https://github.com/syndicalt/llmff --tag v1.0.0`

## Task 11: Post-V1 Guardrails

**Files:**

- Modify: `docs/governance.md`
- Modify: `docs/roadmap.md`
- Modify: `CONTRIBUTING.md`

- [x] **Step 1: Make post-v1 semver practical**

  Document examples of patch, minor, and major changes for:

  - manifest schema
  - CLI flags
  - plugin protocol
  - trace/event schemas
  - library API

- [x] **Step 2: Add deprecation templates**

  Provide a short checklist for deprecating any public surface.

- [x] **Step 3: Validate**

  Run:

  ```bash
  scripts/check-governance-readiness.sh
  ```

  Expected: pass.

## Done Criteria For V1.0

- `cargo fmt --check` passes.
- `cargo clippy --workspace --all-targets -- -D warnings` passes.
- `cargo test --workspace --locked --no-fail-fast` passes.
- Schema, plugin, governance, ecosystem, real-world workflow, and streaming smoke gates pass.
- Public v1.0 contract is documented and reviewed.
- No public machine-readable output lacks schema or fixture coverage.
- Examples are offline-runnable by default unless clearly marked live-provider.
- Plugin protocol v1 support and trust boundaries are documented.
- Provider support tiers are evidence-backed.
- Release artifacts are built, checksummed, and smoke-tested.
- Pre-1.0 migration notes and `v1.0.0` release notes are complete.
