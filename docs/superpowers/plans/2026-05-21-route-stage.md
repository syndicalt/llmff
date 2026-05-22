# Route Stage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement status-based and scalar-field-based `route` stage execution for sequential manifests.

**Architecture:** Extend `StageSpec` with route fields, validate route references in `Graph`, and execute `route` inside `Engine` because it needs access to previous stage statuses. Keep route deterministic and side-effect-free.

**Tech Stack:** Rust standard library, existing manifest/graph/engine tests.

---

## Task 1: Route Manifest Fields

**Files:**
- Modify: `crates/llmff-core/src/manifest.rs`
- Modify: tests constructing `StageSpec`

- [ ] Write a failing manifest parsing test for `on_success`, `on_invalid`, `field`, `cases`, and `default`.
- [ ] Run: `cargo test -p llmff-core manifest::tests::parses_route_fields`
- [ ] Add fields to `StageSpec`.
- [ ] Update existing `StageSpec` constructors.
- [ ] Run: `cargo test -p llmff-core manifest::tests::parses_route_fields`
- [ ] Commit: `feat: parse route fields`

## Task 2: Route Graph Validation

**Files:**
- Modify: `crates/llmff-core/src/graph.rs`

- [ ] Write failing tests for valid route targets and unknown route targets.
- [ ] Run: `cargo test -p llmff-core graph::tests::validates_route_targets graph::tests::rejects_unknown_route_target`
- [ ] Validate route target references against earlier stage ids.
- [ ] Run: `cargo test -p llmff-core graph::tests`
- [ ] Commit: `feat: validate route targets`

## Task 3: Route Engine Execution

**Files:**
- Modify: `crates/llmff-core/src/engine.rs`

- [ ] Write failing engine tests for status success routing and invalid routing.
- [ ] Write failing engine tests for JSON field case routing and default routing.
- [ ] Run: `cargo test -p llmff-core engine::tests::route_`
- [ ] Implement route execution by cloning the selected prior stage status.
- [ ] Run: `cargo test -p llmff-core engine::tests::route_`
- [ ] Commit: `feat: execute route stages`

## Task 4: Example And Docs

**Files:**
- Modify: `examples/json-repair.yaml`
- Modify: `README.md`

- [ ] Update example manifest to route between `validate` and `repair`.
- [ ] Document status and field route forms in README.
- [ ] Run: `cargo test -p llmff --test cli_run`
- [ ] Run mock example and confirm output.
- [ ] Commit: `docs: document route stage`

## Final Verification

- [ ] Run: `cargo fmt --all --check`
- [ ] Run: `cargo test --workspace`
- [ ] Run: `cargo run -p llmff -- inspect examples/json-repair.yaml`
- [ ] Run mock example with trace, confirm `{"answer":"ok"}`, and remove generated output.
