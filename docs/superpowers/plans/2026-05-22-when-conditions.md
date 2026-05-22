# When Conditions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make manifest `when` conditions control stage execution and produce skipped statuses without invoking stage side effects.

**Architecture:** Keep `when` validation in `Engine::validate_stage` because it is stage semantics. Add a uniform pre-dispatch condition check in `Engine::execute_stage` so every current and future stage observes the same skip behavior.

**Tech Stack:** Rust workspace, `llmff-core`, `llmff-cli`, existing `async-trait`, `tempfile`, and trace tests.

---

## File Structure

- Modify `crates/llmff-core/src/engine.rs`: validate `when`, check conditions before dispatch, add tests.
- Modify `crates/llmff-cli/tests/cli_run.rs`: add inspect test for unsupported `when`.
- Modify `README.md`: document `when`.
- Create/modify docs under `docs/superpowers/specs` and `docs/superpowers/plans` for this slice.

## Task 1: Validate `when` Values

**Files:**
- Modify: `crates/llmff-core/src/engine.rs`
- Modify: `crates/llmff-cli/tests/cli_run.rs`

- [x] **Step 1: Write failing core validation test**

Add this test to `crates/llmff-core/src/engine.rs`:

```rust
#[test]
fn validate_manifest_rejects_unknown_when_condition() {
    let manifest = Manifest::from_yaml_str(
        r#"
version: 1
inputs:
  prompt:
    path: prompt.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: load_prompt
    when: maybe
    model: mock:good
"#,
    )
    .unwrap();

    let error = Engine::new()
        .with_backend("mock:good", Arc::new(MockBackend::new("mock:good", "ok")))
        .validate_manifest(manifest)
        .expect_err("unknown when condition should be rejected");

    assert!(error
        .to_string()
        .contains("stage `draft` failed: unknown when condition `maybe`"));
}
```

- [x] **Step 2: Run test to verify RED**

Run:

```bash
cargo test -p llmff-core validate_manifest_rejects_unknown_when_condition
```

Expected: FAIL because unknown `when` values are currently accepted.

- [x] **Step 3: Implement validation**

In `Engine::validate_stage`, call a helper for all stages:

```rust
validate_when_condition(stage)?;
```

Add:

```rust
fn validate_when_condition(stage: &StageSpec) -> Result<(), LlmffError> {
    let Some(condition) = stage.when.as_deref() else {
        return Ok(());
    };
    match condition {
        "success" | "invalid" | "skipped" => {
            if stage.from.is_none() {
                return Err(stage_validation_error(stage, "when requires from"));
            }
            Ok(())
        }
        other => Err(stage_validation_error(
            stage,
            format!("unknown when condition `{other}`"),
        )),
    }
}
```

- [x] **Step 4: Run test to verify GREEN**

Run:

```bash
cargo test -p llmff-core validate_manifest_rejects_unknown_when_condition
```

Expected: PASS.

- [x] **Step 5: Add CLI inspect test and commit**

Add this test to `crates/llmff-cli/tests/cli_run.rs` near other inspect tests:

```rust
#[test]
fn inspect_rejects_unknown_when_condition() {
    let dir = tempfile::tempdir().unwrap();
    let prompt = dir.path().join("question.txt");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&prompt, "Return an answer object").unwrap();
    std::fs::write(
        &manifest,
        format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: load_prompt
    when: maybe
    model: mock:good
"#,
            prompt.display()
        ),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("llmff").unwrap();
    cmd.args(["inspect", manifest.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown when condition `maybe`"));
}
```

Run:

```bash
cargo test -p llmff --test cli_run inspect_rejects_unknown_when_condition
```

Expected: PASS.

Commit:

```bash
git add crates/llmff-core/src/engine.rs crates/llmff-cli/tests/cli_run.rs
git commit -m "feat: validate when conditions"
```

## Task 2: Execute Conditional Stages

**Files:**
- Modify: `crates/llmff-core/src/engine.rs`

- [x] **Step 1: Write failing runtime tests**

Add these tests to `crates/llmff-core/src/engine.rs`:

```rust
#[tokio::test]
async fn when_invalid_skips_stage_on_success_parent() {
    let dir = tempdir().unwrap();
    let prompt_path = dir.path().join("question.txt");
    let output_path = dir.path().join("answer.json");
    std::fs::write(&prompt_path, r#"{"answer":"ok"}"#).unwrap();

    let manifest = Manifest::from_yaml_str(&format!(
        r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: validate
    op: validate_json
    from: load_prompt
    schema: '{{"type":"object","required":["answer"]}}'
  - id: repair
    op: repair
    from: validate
    when: invalid
    model: mock:good
  - id: choose
    op: route
    from: validate
    on_success: validate
    on_invalid: repair
outputs:
  final:
    from: choose
    path: {}
"#,
        prompt_path.display(),
        output_path.display()
    ))
    .unwrap();

    let engine = Engine::new().with_backend(
        "mock:good",
        Arc::new(MockBackend::new("mock:good", r#"{"answer":"repaired"}"#)),
    );

    let report = engine.run_manifest(manifest, dir.path()).await.unwrap();

    assert_eq!(report.final_status, RunStatus::Succeeded);
    assert_eq!(std::fs::read_to_string(output_path).unwrap(), r#"{"answer":"ok"}"#);
}

#[tokio::test]
async fn when_invalid_runs_stage_on_invalid_parent() {
    let dir = tempdir().unwrap();
    let prompt_path = dir.path().join("question.txt");
    let output_path = dir.path().join("answer.json");
    std::fs::write(&prompt_path, r#"{"wrong":true}"#).unwrap();

    let manifest = Manifest::from_yaml_str(&format!(
        r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: validate
    op: validate_json
    from: load_prompt
    schema: '{{"type":"object","required":["answer"]}}'
  - id: repair
    op: repair
    from: validate
    when: invalid
    model: mock:good
  - id: choose
    op: route
    from: validate
    on_success: validate
    on_invalid: repair
outputs:
  final:
    from: choose
    path: {}
"#,
        prompt_path.display(),
        output_path.display()
    ))
    .unwrap();

    let engine = Engine::new().with_backend(
        "mock:good",
        Arc::new(MockBackend::new("mock:good", r#"{"answer":"repaired"}"#)),
    );

    let report = engine.run_manifest(manifest, dir.path()).await.unwrap();

    assert_eq!(report.final_status, RunStatus::Succeeded);
    assert_eq!(
        std::fs::read_to_string(output_path).unwrap(),
        r#"{"answer":"repaired"}"#
    );
}
```

- [x] **Step 2: Run tests to verify RED**

Run:

```bash
cargo test -p llmff-core when_invalid_
```

Expected: at least `when_invalid_skips_stage_on_success_parent` FAILS because the repair stage currently forwards success instead of returning skipped.

- [x] **Step 3: Implement pre-dispatch condition checks**

In `execute_stage`, before the `match stage.op.as_str()` dispatch, add:

```rust
if !should_execute_stage(stage, statuses)? {
    return Ok(StageStatus::Skipped);
}
```

Add helpers:

```rust
fn should_execute_stage(
    stage: &StageSpec,
    statuses: &BTreeMap<String, StageStatus>,
) -> Result<bool, LlmffError> {
    let Some(condition) = stage.when.as_deref() else {
        return Ok(true);
    };
    let parent_id = stage.from.as_ref().ok_or_else(|| LlmffError::StageExecution {
        stage_id: stage.id.clone(),
        message: "when requires parent stage".to_string(),
    })?;
    let parent = statuses.get(parent_id).ok_or_else(|| LlmffError::StageExecution {
        stage_id: stage.id.clone(),
        message: format!("unknown parent stage `{parent_id}`"),
    })?;
    Ok(matches_when(condition, parent))
}

fn matches_when(condition: &str, status: &StageStatus) -> bool {
    matches!(
        (condition, status),
        ("success", StageStatus::Success(_))
            | ("invalid", StageStatus::Invalid { .. })
            | ("skipped", StageStatus::Skipped)
    )
}
```

- [x] **Step 4: Run tests to verify GREEN**

Run:

```bash
cargo test -p llmff-core when_invalid_
```

Expected: PASS.

- [x] **Step 5: Commit**

```bash
git add crates/llmff-core/src/engine.rs
git commit -m "feat: execute when conditions"
```

## Task 3: Trace and Documentation

**Files:**
- Modify: `crates/llmff-core/src/engine.rs`
- Modify: `README.md`
- Modify: `docs/superpowers/specs/2026-05-22-when-conditions-design.md`
- Modify: `docs/superpowers/plans/2026-05-22-when-conditions.md`

- [x] **Step 1: Add trace regression if needed**

If existing trace tests do not cover skipped stages, add a focused test asserting a skipped stage writes `status: "skipped"` in trace JSONL.

- [x] **Step 2: Document `when`**

Update README with a short `when` section near route docs.

- [x] **Step 3: Run final verification**

Run:

```bash
cargo fmt --all --check
cargo test --workspace
cargo run -p llmff -- inspect examples/json-repair.yaml
```

Expected: all commands exit 0; inspect prints `ok`.

- [x] **Step 4: Commit**

```bash
git add crates/llmff-core/src/engine.rs README.md docs/superpowers/specs/2026-05-22-when-conditions-design.md docs/superpowers/plans/2026-05-22-when-conditions.md
git commit -m "docs: document when conditions"
```

## Self-Review

- Spec coverage: validation, runtime skipping, skipped traces, docs, and verification are covered.
- Placeholder scan: no placeholders or open-ended tasks.
- Type consistency: uses existing `StageSpec.when`, `StageStatus::Skipped`, `Engine::validate_manifest`, and `execute_stage`.
