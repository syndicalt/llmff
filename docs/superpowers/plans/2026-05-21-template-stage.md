# Template Stage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the built-in `template` stage using file-backed templates and deterministic placeholder substitution.

**Architecture:** Keep template execution in `llmff-core::stage` as a deterministic stage. Reuse manifest-relative path resolution through the `cwd` already passed by the engine. Add `template` to the engine deterministic stage dispatch.

**Tech Stack:** Rust standard library, existing `serde_json`, existing tests.

---

## Task 1: Unit Template Rendering

**Files:**
- Modify: `crates/llmff-core/src/stage.rs`

- [ ] Write failing tests for `template` text parent substitution, JSON object substitution, and missing variable error.
- [ ] Run: `cargo test -p llmff-core stage::tests::template_stage`
- [ ] Implement file-backed template loading and `{{name}}` substitution.
- [ ] Run: `cargo test -p llmff-core stage::tests::template_stage`
- [ ] Commit: `feat: render file-backed templates`

## Task 2: Engine Dispatch

**Files:**
- Modify: `crates/llmff-core/src/engine.rs`

- [ ] Write a failing engine test proving a manifest with `op: template` runs before `infer`.
- [ ] Run: `cargo test -p llmff-core engine::tests::runs_template_stage_before_infer`
- [ ] Add `template` to deterministic stage dispatch.
- [ ] Run: `cargo test -p llmff-core engine::tests::runs_template_stage_before_infer`
- [ ] Commit: `feat: execute template stages in pipelines`

## Task 3: Examples And README

**Files:**
- Add: `examples/prompt.tmpl`
- Modify: `examples/json-repair.yaml`
- Modify: `README.md`

- [ ] Update the example manifest to use `op: template` after `load_prompt`.
- [ ] Update README to document `{{input}}` and JSON object field substitution.
- [ ] Run: `cargo test -p llmff --test cli_run inspect_example_manifest_succeeds`
- [ ] Run the mock example and confirm `examples/answer.json` contains `{"answer":"ok"}`.
- [ ] Commit: `docs: use template stage in example`

## Final Verification

- [ ] Run: `cargo fmt --all --check`
- [ ] Run: `cargo test --workspace`
- [ ] Run: `cargo run -p llmff -- inspect examples/json-repair.yaml`
- [ ] Run the mock example with trace and remove generated `examples/answer.json`.
