# Trace Metadata Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enrich trace JSONL with timestamps, durations, model metadata, validation errors, and safe operation metadata.

**Architecture:** Extend `TraceEvent` with optional fields and add a small metadata builder in `engine.rs`. Keep trace writing JSONL-compatible and additive.

**Tech Stack:** Rust, serde, serde_json, Tokio tests, tempfile, wiremock.

---

## File Structure

- Modify `crates/llmff-core/src/trace.rs` for trace event fields.
- Modify `crates/llmff-core/src/engine.rs` for metadata emission and tests.
- Modify `README.md` for trace field documentation.

### Task 1: Add Timestamp And Duration Fields

**Files:**
- Modify: `crates/llmff-core/src/trace.rs`
- Modify: `crates/llmff-core/src/engine.rs`

- [ ] **Step 1: Write failing test**

Add an engine test that runs a simple traced pipeline, parses JSONL lines, and asserts every event has `timestamp_ms`, while `stage_finished` has numeric `duration_ms`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p llmff-core engine::tests::trace_events_include_timestamps_and_stage_durations`

Expected: FAIL because trace events do not include those fields.

- [ ] **Step 3: Write minimal implementation**

Add optional fields to `TraceEvent`:

```rust
pub timestamp_ms: u128,
#[serde(skip_serializing_if = "Option::is_none")]
pub duration_ms: Option<u128>,
```

Set timestamps from `SystemTime::now()` and stage duration from `Instant::now()`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p llmff-core engine::tests::trace_events_include_timestamps_and_stage_durations`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/llmff-core/src/trace.rs crates/llmff-core/src/engine.rs
git commit -m "feat: trace timestamps and durations"
```

### Task 2: Add Stage Metadata Fields

**Files:**
- Modify: `crates/llmff-core/src/trace.rs`
- Modify: `crates/llmff-core/src/engine.rs`

- [ ] **Step 1: Write failing tests**

Add tests proving:

- `infer` stage finished trace includes `model`, `backend`, and `provider_model`.
- invalid `validate_json` stage finished trace includes `validation_errors`.
- command `tool` and `write` stage finished traces include safe operation metadata.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p llmff-core engine::tests::trace_events_include`

Expected: FAIL because trace events do not include these metadata fields.

- [ ] **Step 3: Write minimal implementation**

Add optional `TraceEvent` fields and a helper that derives metadata from `StageSpec` and `StageStatus`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p llmff-core engine::tests::trace_events_include`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/llmff-core/src/trace.rs crates/llmff-core/src/engine.rs
git commit -m "feat: trace stage metadata"
```

### Task 3: Document And Verify

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Document trace metadata**

Add a concise list of trace fields and note that payloads/secrets are not traced.

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
git add README.md docs/superpowers/specs/2026-05-22-trace-metadata-design.md docs/superpowers/plans/2026-05-22-trace-metadata.md
git commit -m "docs: document trace metadata"
```
