# Cache Stage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a deterministic file-backed `cache` stage for reusing successful pipeline values across runs.

**Architecture:** Extend `StageSpec` with a cache namespace field, execute cache in the core engine because it needs parent status and trace metadata, and store typed values as versioned JSON cache records. Keep the feature CLI-first with explicit manifest fields and no mandatory environment variables.

**Tech Stack:** Rust workspace, `serde`, `serde_json`, `sha2`, existing manifest/engine/CLI tests, tempdirs for cache fixtures.

---

## File Structure

- Modify `Cargo.toml`: add workspace dependency `sha2`.
- Modify `crates/llmff-core/Cargo.toml`: depend on `sha2`.
- Modify `crates/llmff-core/src/manifest.rs`: add `key: Option<String>` to `StageSpec` and a parsing test.
- Modify `crates/llmff-core/src/inline_graph.rs`: initialize `key: None` for inline-created stages.
- Modify `crates/llmff-core/src/engine.rs`: validate and execute `cache`, compute cache keys, read/write cache records, infer value kind, and attach trace metadata.
- Modify `crates/llmff-core/src/trace.rs`: add optional `cache_hit` and `cache_path` fields.
- Modify `crates/llmff-cli/src/commands.rs`: include `cache` in `stages list`.
- Modify `crates/llmff-cli/tests/cli_run.rs`: add stage-list and end-to-end cache coverage.
- Modify `README.md`: document `cache` and update limitations.

## Task 1: Manifest and Dependency Wiring

- [ ] **Step 1: Write failing manifest cache-key test**

Add `parses_cache_fields` in `crates/llmff-core/src/manifest.rs`:

```rust
#[test]
fn parses_cache_fields() {
    let yaml = r#"
version: 1
graph:
  - id: cached_prompt
    op: cache
    from: render_prompt
    path: .llmff/cache
    key: prompt-v1
"#;

    let manifest = Manifest::from_yaml_str(yaml).expect("manifest should parse");
    let stage = &manifest.graph[0];

    assert_eq!(stage.path.as_deref(), Some(".llmff/cache"));
    assert_eq!(stage.key.as_deref(), Some("prompt-v1"));
}
```

- [ ] **Step 2: Run RED**

Run:

```bash
cargo test -p llmff-core manifest::tests::parses_cache_fields
```

Expected: FAIL because `StageSpec` has no `key` field.

- [ ] **Step 3: Implement manifest field and dependency wiring**

Add to `StageSpec`:

```rust
pub key: Option<String>,
```

Initialize `key: None` in every existing `StageSpec` literal and in `empty_stage()` in `crates/llmff-core/src/inline_graph.rs`.

Add `sha2 = "0.10"` to workspace dependencies and `sha2.workspace = true` to `llmff-core`.

- [ ] **Step 4: Run GREEN and commit**

Run:

```bash
cargo test -p llmff-core manifest::tests::parses_cache_fields
```

Commit:

```bash
git add Cargo.toml Cargo.lock crates/llmff-core/Cargo.toml crates/llmff-core/src/manifest.rs crates/llmff-core/src/inline_graph.rs crates/llmff-core/src/*.rs docs/superpowers/specs/2026-05-22-cache-stage-design.md docs/superpowers/plans/2026-05-22-cache-stage.md
git commit -m "feat: parse cache stage fields"
```

## Task 2: Core Cache Execution

- [ ] **Step 1: Write failing validation and execution tests**

Add tests in `crates/llmff-core/src/engine.rs`:

```rust
#[tokio::test]
async fn cache_stage_writes_and_reuses_success_value() {
    let dir = tempfile::tempdir().unwrap();
    let first = cache_manifest("first", "answer-v1");
    let second = cache_manifest("second", "answer-v1");
    let engine = Engine::new();

    engine
        .run_manifest(first, dir.path())
        .await
        .expect("first run should populate cache");
    let output = std::fs::read_to_string(dir.path().join("answer.txt")).unwrap();
    assert_eq!(output, "first");

    engine
        .run_manifest(second, dir.path())
        .await
        .expect("second run should read cache");
    let output = std::fs::read_to_string(dir.path().join("answer.txt")).unwrap();
    assert_eq!(output, "first");
}

#[test]
fn validate_manifest_rejects_cache_without_parent() {
    let manifest = Manifest::from_yaml_str(
        r#"
version: 1
graph:
  - id: cached
    op: cache
"#,
    )
    .unwrap();

    let error = Engine::new()
        .validate_manifest(manifest)
        .expect_err("cache without parent should be rejected");

    assert!(error
        .to_string()
        .contains("stage `cached` failed: cache requires from"));
}
```

Add helper:

```rust
fn cache_manifest(text: &str, key: &str) -> Manifest {
    Manifest::from_yaml_str(&format!(
        r#"
version: 1
inputs:
  prompt:
    path: prompt.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: cached
    op: cache
    from: load_prompt
    path: .llmff/cache
    key: {key}
outputs:
  final:
    from: cached
    path: answer.txt
"#,
    ))
    .expect("manifest should parse")
}
```

Before each run in the test, write `prompt.txt` with `text`.

- [ ] **Step 2: Run RED**

Run:

```bash
cargo test -p llmff-core cache_stage validate_manifest_rejects_cache_without_parent
```

Expected: FAIL because `cache` is unknown.

- [ ] **Step 3: Implement cache execution**

In `engine.rs`:

- Add `"cache"` validation requiring `from`.
- Add `"cache"` dispatch to `execute_stage`.
- Add `execute_cache(stage, statuses, cwd) -> Result<StageOutcome, LlmffError>`.
- Create `CacheRecord { version: u32, value: Value }`.
- Compute the SHA-256 digest over canonical JSON containing version, stage id, namespace, and parent value.
- Write cache files as pretty JSON through a temporary file and `std::fs::rename`.
- Return `StageOutcome` with `cache_hit: Some(true | false)`.
- Infer cache output kind from its parent.

- [ ] **Step 4: Run GREEN and commit**

Run:

```bash
cargo test -p llmff-core cache_stage validate_manifest_rejects_cache_without_parent
```

Commit:

```bash
git add Cargo.toml Cargo.lock crates/llmff-core/Cargo.toml crates/llmff-core/src/engine.rs crates/llmff-core/src/trace.rs
git commit -m "feat: execute cache stages"
```

## Task 3: Trace, CLI, and Documentation

- [ ] **Step 1: Write failing trace and CLI tests**

Add a core trace test that runs the same cache manifest twice with `RunOptions.trace_path`, then asserts one `cache_hit:false` and one `cache_hit:true` stage-finished event for `cached`.

Update `stages_list_prints_builtin_stages` to assert `cache`.

Add `run_executes_cache_stage` in `cli_run.rs` with the same two-run behavior as the core test.

- [ ] **Step 2: Run RED**

Run:

```bash
cargo test -p llmff-core trace_events_include_cache_metadata
cargo test -p llmff --test cli_run stages_list_prints_builtin_stages run_executes_cache_stage
```

Expected: FAIL until trace metadata and stage listing are wired.

- [ ] **Step 3: Implement CLI/docs**

Add `cache_hit` and `cache_path` to `TraceEvent` and trace summary output. Add `cache` to `llmff stages list`.

Document:

- cache manifest form
- default `.llmff/cache`
- stable key inputs
- no required env flags
- remove `cache stages` from limitations

- [ ] **Step 4: Run GREEN and final verification**

Run:

```bash
cargo test -p llmff-core trace_events_include_cache_metadata
cargo test -p llmff --test cli_run stages_list_prints_builtin_stages run_executes_cache_stage
cargo fmt --all --check
cargo test --workspace
cargo run -p llmff -- inspect examples/json-repair.yaml
```

- [ ] **Step 5: Commit**

Commit:

```bash
git add README.md crates/llmff-cli/src/commands.rs crates/llmff-cli/tests/cli_run.rs crates/llmff-core/src/engine.rs crates/llmff-core/src/trace.rs docs/superpowers/plans/2026-05-22-cache-stage.md
git commit -m "docs: document cache stage"
```

## Self-Review

- Spec coverage: manifest parsing, validation, execution, stable typed cache records, trace metadata, CLI visibility, docs, and verification are covered.
- Placeholder scan: no placeholder implementation steps remain.
- Type consistency: the plan uses `key`, `path`, `cache_hit`, and `cache_path` consistently with the design.
