# Retrieve Stage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a deterministic file-backed `retrieve` stage for simple lexical retrieval inside `llmff` graphs.

**Architecture:** Extend `StageSpec` with retrieval parameters, implement retrieval as a deterministic stage in `stage.rs`, wire validation and type inference in `engine.rs`, and expose the stage through CLI stage listing and README docs. Keep this slice local and deterministic; no vector DB, embeddings, globbing, or plugins.

**Tech Stack:** Rust workspace, `serde`, `serde_json`, existing manifest/stage/engine/CLI tests, filesystem tempdirs for deterministic retrieval fixtures.

---

## File Structure

- Modify `crates/llmff-core/src/manifest.rs`: add `documents` and `top_k` fields to `StageSpec`.
- Modify `crates/llmff-core/src/stage.rs`: implement `retrieve`.
- Modify `crates/llmff-core/src/engine.rs`: validate retrieve fields, dispatch deterministic stage, and infer JSON output kind.
- Modify `crates/llmff-core/src/inline_graph.rs`: initialize new fields.
- Modify `crates/llmff-cli/src/commands.rs`: include `retrieve` in `stages list`.
- Modify `crates/llmff-cli/tests/cli_run.rs`: add retrieve CLI integration coverage.
- Modify `README.md`: document retrieve and update limitations.

## Task 1: Manifest and Core Stage

- [x] **Step 1: Write failing manifest test**

Add a manifest test proving `documents: [docs/a.txt, docs/b.txt]` and `top_k: 1` parse into a `retrieve` stage.

- [x] **Step 2: Run RED**

Run `cargo test -p llmff-core manifest::tests::parses_retrieve_fields`.

Expected: FAIL because `StageSpec` has no `documents` or `top_k`.

- [x] **Step 3: Implement manifest fields**

Add `#[serde(default)] pub documents: Vec<String>` and `pub top_k: Option<usize>` to `StageSpec`, and initialize these fields in inline graph stage construction and existing tests.

- [x] **Step 4: Run GREEN and commit**

Run `cargo test -p llmff-core manifest::tests::parses_retrieve_fields`.

Commit `feat: parse retrieve stage fields`.

## Task 2: Retrieve Execution and Validation

- [ ] **Step 1: Write failing stage retrieval test**

Add `retrieve_stage_returns_top_lexical_matches` in `stage.rs`. It should create two documents, query for `rust graph`, set `top_k: 1`, and expect one JSON match for the Rust graph document.

- [ ] **Step 2: Write failing engine validation tests**

Add tests rejecting `retrieve` without `from` and without `documents`.

- [ ] **Step 3: Run RED**

Run `cargo test -p llmff-core retrieve`.

Expected: FAIL because `retrieve` is unknown and validation does not know its fields.

- [ ] **Step 4: Implement retrieve**

Add retrieve dispatch to deterministic stages. Implement lexical tokenization, scoring, stable sorting, `top_k`, and JSON output. Add engine validation and JSON type inference.

- [ ] **Step 5: Run GREEN and commit**

Run `cargo test -p llmff-core retrieve`.

Commit `feat: add file-backed retrieve stage`.

## Task 3: CLI and Documentation

- [ ] **Step 1: Write failing CLI test**

Add a CLI test that runs a manifest with `retrieve` and writes the JSON output.

- [ ] **Step 2: Run RED**

Run `cargo test -p llmff --test cli_run run_executes_retrieve_stage`.

Expected: FAIL until the stage is fully wired into CLI-visible execution.

- [ ] **Step 3: Implement CLI/docs**

Add `retrieve` to `stages list`; document the stage in README and remove retrieval from the limitations bullet.

- [ ] **Step 4: Run GREEN and final verification**

Run focused CLI test, `cargo fmt --all --check`, `cargo test --workspace`, and `cargo run -p llmff -- inspect examples/json-repair.yaml`.

- [ ] **Step 5: Commit**

Commit `docs: document retrieve stage`.

## Self-Review

- Spec coverage: parsing, execution, validation, CLI visibility, docs, and verification are covered.
- Placeholder scan: no placeholder implementation steps remain.
- Type consistency: plan uses `documents`, `top_k`, and `retrieve` consistently.
