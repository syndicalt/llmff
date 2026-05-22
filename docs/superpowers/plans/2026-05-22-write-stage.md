# Write Stage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement `op: write` as a first-class graph stage that persists and forwards successful values.

**Architecture:** Reuse the engine's existing serialization and output path logic. Add graph validation for required `path`, then add engine dispatch for `write`.

**Tech Stack:** Rust, Tokio tests, tempfile.

---

## File Structure

- Modify `crates/llmff-core/src/graph.rs` for required `path` validation.
- Modify `crates/llmff-core/src/engine.rs` for write-stage execution.
- Modify `README.md` for a `write` stage example.

### Task 1: Validate Write Stage Path

**Files:**
- Modify: `crates/llmff-core/src/graph.rs`

- [ ] **Step 1: Write the failing test**

Add a graph test that rejects:

```yaml
graph:
  - id: source
    op: load
  - id: save
    op: write
    from: source
```

Expected error substring: `write requires path`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p llmff-core graph::tests::rejects_write_without_path`

Expected: FAIL because graph validation currently accepts the manifest.

- [ ] **Step 3: Write minimal implementation**

Add `validate_write_stage(stage)?` inside the graph loop. It should only apply to `op: write` and require `path`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p llmff-core graph::tests::rejects_write_without_path`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/llmff-core/src/graph.rs
git commit -m "feat: validate write stage configuration"
```

### Task 2: Execute Write Stage

**Files:**
- Modify: `crates/llmff-core/src/engine.rs`

- [ ] **Step 1: Write the failing test**

Add an engine test proving `write` writes its parent value to `path` and forwards the same value to a top-level output.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p llmff-core engine::tests::write_stage_writes_and_forwards_parent_value`

Expected: FAIL because `write` is currently an unknown stage.

- [ ] **Step 3: Write minimal implementation**

Add `"write" => self.execute_write(stage, statuses, cwd)` to engine dispatch. The implementation should require a successful parent, call existing `write_output`, and return `StageStatus::Success(value.clone())`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p llmff-core engine::tests::write_stage_writes_and_forwards_parent_value`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/llmff-core/src/engine.rs
git commit -m "feat: execute write stages"
```

### Task 3: Document And Verify

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Add README example**

Document `op: write` with `path` and note that top-level `outputs` still works.

- [ ] **Step 2: Run full verification**

Run:

```bash
cargo fmt --all --check
cargo test --workspace
cargo run -p llmff -- inspect examples/json-repair.yaml
```

Expected: all commands exit 0.

- [ ] **Step 3: Commit**

```bash
git add README.md docs/superpowers/specs/2026-05-22-write-stage-design.md docs/superpowers/plans/2026-05-22-write-stage.md
git commit -m "docs: document write stage"
```
