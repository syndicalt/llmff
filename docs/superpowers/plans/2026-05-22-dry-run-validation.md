# Dry-Run Validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `llmff inspect` a real dry-run validator that catches missing stage requirements and backend configuration without invoking models or tools.

**Architecture:** Keep graph-only reference checks in `llmff-core::graph`, and add `Engine::validate_manifest` for operation, parameter, and backend checks that require engine state. Refactor `llmff-cli::commands` so `run` and `inspect` share the same explicit backend registry builder.

**Tech Stack:** Rust workspace, `llmff-core`, `llmff-cli`, `clap`, `assert_cmd`, `predicates`, `tokio`.

---

## File Structure

- Modify `crates/llmff-core/src/engine.rs`: add `validate_manifest`, stage requirement validation helpers, and route target presence validation.
- Modify `crates/llmff-cli/src/commands.rs`: add backend flags to `inspect`, extract shared `build_engine`, and call engine validation.
- Modify `crates/llmff-cli/tests/cli_run.rs`: add inspect integration tests for unresolved and registered backend aliases.
- Modify `README.md`: document inspect as dry-run validation and show backend flags.

## Task 1: Engine Dry-Run Validation

**Files:**
- Modify: `crates/llmff-core/src/engine.rs`
- Test: `crates/llmff-core/src/engine.rs`

- [x] **Step 1: Write failing engine tests**

Add these tests to the existing `#[cfg(test)] mod tests` in `crates/llmff-core/src/engine.rs`:

```rust
#[test]
fn validate_manifest_rejects_unknown_stage_operation() {
    let manifest = Manifest::from_yaml_str(
        r#"
version: 1
graph:
  - id: mystery
    op: unknown_op
"#,
    )
    .unwrap();

    let error = Engine::new()
        .validate_manifest(manifest)
        .expect_err("unknown stage operation should be rejected");

    assert!(error.to_string().contains("unknown stage operation `unknown_op`"));
}

#[test]
fn validate_manifest_rejects_missing_required_stage_parameters() {
    let manifest = Manifest::from_yaml_str(
        r#"
version: 1
graph:
  - id: prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: prompt
  - id: validate
    op: validate_json
    from: draft
"#,
    )
    .unwrap();

    let error = Engine::new()
        .validate_manifest(manifest)
        .expect_err("missing infer model should be rejected first");

    assert!(error.to_string().contains("stage `draft` failed: infer requires model"));
}

#[test]
fn validate_manifest_rejects_missing_backend_without_calling_it() {
    let manifest = Manifest::from_yaml_str(
        r#"
version: 1
inputs:
  prompt:
    path: prompt.txt
graph:
  - id: prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: prompt
    model: openai:gpt-test
"#,
    )
    .unwrap();

    let error = Engine::new()
        .validate_manifest(manifest)
        .expect_err("unregistered backend alias should be rejected");

    assert!(error.to_string().contains("no backend configured for `openai:gpt-test`"));
}
```

- [x] **Step 2: Run tests to verify RED**

Run:

```bash
cargo test -p llmff-core validate_manifest_
```

Expected: FAIL because `Engine::validate_manifest` does not exist.

- [x] **Step 3: Implement minimal engine validation**

Add `pub fn validate_manifest(&self, manifest: Manifest) -> Result<Graph, LlmffError>` to `impl Engine`, call it from `run_manifest_with_options`, and add private helpers:

```rust
pub fn validate_manifest(&self, manifest: Manifest) -> Result<Graph, LlmffError> {
    let graph = Graph::from_manifest(manifest)?;
    for stage in graph.stages() {
        self.validate_stage(stage)?;
    }
    Ok(graph)
}

fn validate_stage(&self, stage: &StageSpec) -> Result<(), LlmffError> {
    match stage.op.as_str() {
        "load" => require_present(stage, stage.input.as_deref(), "load requires input"),
        "infer" => {
            require_present(stage, stage.from.as_deref(), "infer requires from")?;
            let model = require_present(stage, stage.model.as_deref(), "infer requires model")?;
            self.backend_for_model(model).map(|_| ())
        }
        "validate_json" => {
            require_present(stage, stage.from.as_deref(), "validate_json requires from")?;
            if stage.schema.is_none() && stage.schema_path.is_none() {
                return Err(stage_validation_error(stage, "validate_json requires schema or schema_path"));
            }
            Ok(())
        }
        "system" => require_present(stage, stage.from.as_deref(), "system requires from"),
        "template" => {
            require_present(stage, stage.from.as_deref(), "template requires from")?;
            require_present(stage, stage.path.as_deref(), "template requires path").map(|_| ())
        }
        "repair" => {
            require_present(stage, stage.from.as_deref(), "repair requires from")?;
            let model = require_present(stage, stage.model.as_deref(), "repair requires model")?;
            self.backend_for_model(model).map(|_| ())
        }
        "route" => {
            require_present(stage, stage.from.as_deref(), "route requires from")?;
            if stage.on_success.is_none()
                && stage.on_invalid.is_none()
                && stage.on_skipped.is_none()
                && stage.cases.is_empty()
                && stage.default.is_none()
            {
                return Err(stage_validation_error(stage, "route requires at least one target"));
            }
            Ok(())
        }
        "tool" => require_present(stage, stage.from.as_deref(), "tool requires from"),
        "write" => require_present(stage, stage.from.as_deref(), "write requires from"),
        other => Err(LlmffError::UnknownStage(other.to_string())),
    }
}
```

Use small private helpers returning `LlmffError::StageExecution` for stage requirement errors.

- [x] **Step 4: Run tests to verify GREEN**

Run:

```bash
cargo test -p llmff-core validate_manifest_
```

Expected: PASS.

- [x] **Step 5: Commit**

```bash
git add crates/llmff-core/src/engine.rs
git commit -m "feat: validate manifests without execution"
```

## Task 2: Inspect Backend Availability

**Files:**
- Modify: `crates/llmff-cli/src/commands.rs`
- Test: `crates/llmff-cli/tests/cli_run.rs`

- [x] **Step 1: Write failing CLI tests**

Add these tests near `inspect_example_manifest_succeeds`:

```rust
#[test]
fn inspect_rejects_unregistered_backend_alias() {
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
    model: openai:gpt-test
"#,
            prompt.display()
        ),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("llmff").unwrap();
    cmd.args(["inspect", manifest.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "no backend configured for `openai:gpt-test`",
        ));
}

#[test]
fn inspect_accepts_registered_openai_backend_without_calling_server() {
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
    model: openai:gpt-test
"#,
            prompt.display()
        ),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("llmff").unwrap();
    cmd.args([
        "inspect",
        manifest.to_str().unwrap(),
        "--backend",
        "openai=http://127.0.0.1:1",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("ok"));
}
```

- [x] **Step 2: Run tests to verify RED**

Run:

```bash
cargo test -p llmff --test cli_run inspect_
```

Expected: FAIL because `inspect` does not accept backend flags and does not use engine validation.

- [x] **Step 3: Implement shared CLI engine construction**

In `crates/llmff-cli/src/commands.rs`, add the same backend flags to `Command::Inspect`, route them through the match arm, extract `build_engine`, and replace direct graph validation with `engine.validate_manifest(manifest)?`.

The helper should preserve existing mock defaults:

```rust
fn build_engine(
    backend: Vec<String>,
    ollama: Vec<String>,
    api_key_env: Vec<String>,
    api_key: Vec<String>,
) -> Result<Engine> {
    let bad = std::env::var("LLMFF_MOCK_BAD_RESPONSE").unwrap_or_else(|_| "{}".to_string());
    let good = std::env::var("LLMFF_MOCK_GOOD_RESPONSE").unwrap_or_else(|_| bad.clone());
    let mut engine = Engine::new()
        .with_backend("mock:bad", Arc::new(MockBackend::new("mock:bad", bad)))
        .with_backend("mock:good", Arc::new(MockBackend::new("mock:good", good.clone())))
        .with_backend("mock:json", Arc::new(MockBackend::new("mock:json", good)));

    let api_key_env = parse_alias_value_map(api_key_env)?;
    let api_key = parse_alias_value_map(api_key)?;
    for backend in parse_alias_value_list(backend)? {
        let key = api_key
            .get(&backend.alias)
            .cloned()
            .map(Ok)
            .or_else(|| resolve_api_key_env(&api_key_env, &backend.alias))
            .transpose()?
            .unwrap_or_default();
        engine = engine.with_backend(
            backend.alias,
            Arc::new(OpenAiCompatibleBackend::new(backend.value, key)),
        );
    }
    for backend in parse_alias_value_list(ollama)? {
        engine = engine.with_backend(backend.alias, Arc::new(OllamaBackend::new(backend.value)));
    }

    Ok(engine)
}
```

- [x] **Step 4: Run tests to verify GREEN**

Run:

```bash
cargo test -p llmff --test cli_run inspect_
```

Expected: PASS.

- [x] **Step 5: Commit**

```bash
git add crates/llmff-cli/src/commands.rs crates/llmff-cli/tests/cli_run.rs
git commit -m "feat: inspect backend availability"
```

## Task 3: Documentation and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/superpowers/specs/2026-05-22-dry-run-validation-design.md`
- Modify: `docs/superpowers/plans/2026-05-22-dry-run-validation.md`

- [x] **Step 1: Update README**

Document that `inspect` performs dry-run validation and accepts the same explicit backend registration flags as `run`.

- [x] **Step 2: Run final verification**

Run:

```bash
cargo fmt --all --check
cargo test --workspace
cargo run -p llmff -- inspect examples/json-repair.yaml
cargo run -p llmff -- inspect examples/json-repair.yaml --backend openai=http://127.0.0.1:1
```

Expected: all commands exit 0, and inspect prints `ok`.

- [ ] **Step 3: Commit**

```bash
git add README.md docs/superpowers/specs/2026-05-22-dry-run-validation-design.md docs/superpowers/plans/2026-05-22-dry-run-validation.md
git commit -m "docs: document dry-run validation"
```

## Self-Review

- Spec coverage: unknown stages, required parameters, backend availability, no model/tool invocation, CLI-first backend flags, and docs are covered.
- Placeholder scan: no TBD or implementation-later placeholders.
- Type consistency: `Engine::validate_manifest`, `Graph`, `Manifest`, `StageSpec`, and existing CLI flag helper names match current code.
