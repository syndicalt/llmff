# Stage Type Validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add conservative dry-run stage type validation so impossible field-route graphs fail during `inspect` and engine validation.

**Architecture:** Keep structural graph validation in `llmff-core::graph`. Add a small static success-kind inference pass inside `llmff-core::engine::Engine::validate_manifest`, after operation/parameter/backend checks and before returning the normalized graph.

**Tech Stack:** Rust workspace, `llmff-core`, `llmff-cli`, existing `assert_cmd`, `predicates`, and `tempfile` tests.

---

## File Structure

- Modify `crates/llmff-core/src/engine.rs`: add `StageValueKind`, static success-kind inference, field-route compatibility validation, and unit tests.
- Modify `crates/llmff-cli/tests/cli_run.rs`: add an `inspect` integration test for the invalid field route.
- Modify `README.md`: document that `inspect` performs conservative type compatibility validation.
- Create/modify docs under `docs/superpowers/specs` and `docs/superpowers/plans` for this slice.

## Task 1: Engine Type Compatibility Validation

**Files:**
- Modify: `crates/llmff-core/src/engine.rs`
- Test: `crates/llmff-core/src/engine.rs`

- [ ] **Step 1: Write failing engine tests**

Add these tests to the existing `#[cfg(test)] mod tests` in `crates/llmff-core/src/engine.rs`:

```rust
#[test]
fn validate_manifest_rejects_field_route_from_text_source() {
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
  - id: fast_answer
    op: template
    from: load_prompt
    path: fast.tmpl
  - id: choose
    op: route
    from: load_prompt
    field: kind
    cases:
      simple: fast_answer
"#,
    )
    .unwrap();

    let error = Engine::new()
        .validate_manifest(manifest)
        .expect_err("field route from text source should be rejected");

    assert!(error
        .to_string()
        .contains("stage `choose` failed: field route requires JSON source `load_prompt`, got text"));
}

#[test]
fn validate_manifest_accepts_field_route_from_json_source() {
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
  - id: validate
    op: validate_json
    from: load_prompt
    schema: '{"type":"object","required":["kind"]}'
  - id: fast_answer
    op: template
    from: load_prompt
    path: fast.tmpl
  - id: choose
    op: route
    from: validate
    field: kind
    cases:
      simple: fast_answer
"#,
    )
    .unwrap();

    Engine::new()
        .validate_manifest(manifest)
        .expect("field route from validate_json should validate");
}
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
cargo test -p llmff-core validate_manifest_rejects_field_route_from_text_source validate_manifest_accepts_field_route_from_json_source
```

Cargo accepts one name filter at a time, so run these if needed:

```bash
cargo test -p llmff-core validate_manifest_rejects_field_route_from_text_source
cargo test -p llmff-core validate_manifest_accepts_field_route_from_json_source
```

Expected: the reject test FAILS because current dry-run validation accepts the invalid field route.

- [ ] **Step 3: Implement static success-kind validation**

In `crates/llmff-core/src/engine.rs`:

- Add a private enum:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StageValueKind {
    Any,
    Text,
    Json,
}
```

- Add `validate_stage_types(&self, graph: &Graph) -> Result<(), LlmffError>`.
- Call it from `Engine::validate_manifest` after `validate_stage`.
- Track a `BTreeMap<String, StageValueKind>` in graph order.
- For a route with `field`, check `stage.from` kind. If it is `Text`, return:

```rust
stage_validation_error(
    stage,
    format!("field route requires JSON source `{source_id}`, got text"),
)
```

- Infer output kind with:
  - `load`, `system`, `template`, `infer`, `repair`, `tool`: `Text`
  - `validate_json`: `Json`
  - `route`: `Any`
  - `write`: parent kind or `Any`

- [ ] **Step 4: Run tests to verify GREEN**

Run:

```bash
cargo test -p llmff-core validate_manifest_rejects_field_route_from_text_source
cargo test -p llmff-core validate_manifest_accepts_field_route_from_json_source
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/llmff-core/src/engine.rs
git commit -m "feat: validate field route input types"
```

## Task 2: CLI Inspect Type Validation

**Files:**
- Modify: `crates/llmff-cli/tests/cli_run.rs`

- [ ] **Step 1: Write failing CLI integration test**

Add this test near the existing `inspect_*` tests:

```rust
#[test]
fn inspect_rejects_field_route_from_text_source() {
    let dir = tempfile::tempdir().unwrap();
    let prompt = dir.path().join("question.txt");
    let template = dir.path().join("fast.tmpl");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&prompt, r#"{"kind":"simple"}"#).unwrap();
    std::fs::write(&template, "fast").unwrap();
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
  - id: fast_answer
    op: template
    from: load_prompt
    path: {}
  - id: choose
    op: route
    from: load_prompt
    field: kind
    cases:
      simple: fast_answer
"#,
            prompt.display(),
            template.display()
        ),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("llmff").unwrap();
    cmd.args(["inspect", manifest.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "field route requires JSON source `load_prompt`, got text",
        ));
}
```

- [ ] **Step 2: Run test**

Run:

```bash
cargo test -p llmff --test cli_run inspect_rejects_field_route_from_text_source
```

Expected: PASS after Task 1. If it fails before Task 1, that is the expected red state.

- [ ] **Step 3: Commit**

```bash
git add crates/llmff-cli/tests/cli_run.rs
git commit -m "test: inspect rejects field route type mismatch"
```

## Task 3: Documentation and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/superpowers/specs/2026-05-22-stage-type-validation-design.md`
- Modify: `docs/superpowers/plans/2026-05-22-stage-type-validation.md`

- [ ] **Step 1: Update README**

Update the inspect description to mention conservative type compatibility validation.

- [ ] **Step 2: Run final verification**

Run:

```bash
cargo fmt --all --check
cargo test --workspace
cargo run -p llmff -- inspect examples/json-repair.yaml
```

Expected: all commands exit 0, and inspect prints `ok`.

- [ ] **Step 3: Commit**

```bash
git add README.md docs/superpowers/specs/2026-05-22-stage-type-validation-design.md docs/superpowers/plans/2026-05-22-stage-type-validation.md
git commit -m "docs: document stage type validation"
```

## Self-Review

- Spec coverage: static value kinds, field route mismatch rejection, accepted JSON source, CLI inspect behavior, docs, and verification are covered.
- Placeholder scan: no placeholders or open-ended implementation steps.
- Type consistency: uses existing `Engine::validate_manifest`, `Graph`, `StageSpec`, `Manifest`, and `LlmffError` names.
