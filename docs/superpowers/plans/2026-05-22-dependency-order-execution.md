# Dependency-Order Execution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Execute and inspect manifests by graph dependency order instead of manifest order, while keeping execution sequential and deterministic.

**Architecture:** Update `llmff-core::graph::Graph` to validate references against the full stage id set and store a topologically sorted stage list. Leave the engine and CLI consuming `Graph::stages()` so the scheduler improvement stays in the core graph normalization boundary.

**Tech Stack:** Rust workspace, `llmff-core`, `llmff-cli`, `serde_yaml`, existing `tempfile` and mock backend tests.

---

## File Structure

- Modify `crates/llmff-core/src/graph.rs`: full-reference validation, dependency extraction, deterministic topological sort, graph tests.
- Modify `crates/llmff-core/src/engine.rs`: engine integration test proving out-of-order manifests run.
- Modify `README.md`: document dependency-order execution and update limitations.
- Modify `docs/superpowers/specs/2026-05-21-route-stage-design.md`: note that the older earlier-target restriction was superseded.

## Task 1: Graph Dependency Ordering

**Files:**
- Modify: `crates/llmff-core/src/graph.rs`
- Test: `crates/llmff-core/src/graph.rs`

- [x] **Step 1: Write failing graph tests**

Add these tests to `crates/llmff-core/src/graph.rs`:

```rust
#[test]
fn orders_forward_stage_references_by_dependency() {
    let manifest = Manifest::from_yaml_str(
        r#"
version: 1
inputs:
  prompt:
    path: ./question.txt
graph:
  - id: draft
    op: infer
    from: load_prompt
    model: mock:json
  - id: load_prompt
    op: load
    input: prompt
outputs:
  final:
    from: draft
    path: ./answer.json
"#,
    )
    .unwrap();

    let graph = Graph::from_manifest(manifest).expect("forward references should validate");
    let stage_ids = graph
        .stages()
        .iter()
        .map(|stage| stage.id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(stage_ids, vec!["load_prompt", "draft"]);
}

#[test]
fn orders_forward_route_targets_before_route_stage() {
    let manifest = Manifest::from_yaml_str(
        r#"
version: 1
inputs:
  prompt:
    path: ./question.txt
graph:
  - id: choose
    op: route
    from: validate
    on_success: validate
    on_invalid: repair
  - id: repair
    op: repair
    from: validate
    model: mock:good
  - id: validate
    op: validate_json
    from: draft
    schema: '{"type":"object","required":["answer"]}'
  - id: draft
    op: infer
    from: load_prompt
    model: mock:bad
  - id: load_prompt
    op: load
    input: prompt
outputs:
  final:
    from: choose
    path: ./answer.json
"#,
    )
    .unwrap();

    let graph = Graph::from_manifest(manifest).expect("forward route targets should validate");
    let stage_ids = graph
        .stages()
        .iter()
        .map(|stage| stage.id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        stage_ids,
        vec!["load_prompt", "draft", "validate", "repair", "choose"]
    );
}

#[test]
fn rejects_stage_reference_cycle() {
    let manifest = Manifest::from_yaml_str(
        r#"
version: 1
graph:
  - id: first
    op: template
    from: second
    path: prompt.tmpl
  - id: second
    op: template
    from: first
    path: prompt.tmpl
"#,
    )
    .unwrap();

    let error = Graph::from_manifest(manifest).unwrap_err().to_string();

    assert!(error.contains("cycle detected in graph"));
}
```

- [x] **Step 2: Run tests to verify RED**

Run:

```bash
cargo test -p llmff-core graph::tests::orders_forward graph::tests::rejects_stage_reference_cycle
```

Expected: FAIL because graph validation currently rejects forward references before topological ordering.

- [x] **Step 3: Implement full-reference validation and topological sort**

In `crates/llmff-core/src/graph.rs`:

- Collect all stage ids before validating references.
- Validate `input`, `from`, route target, output, tool, and write rules against full sets.
- Add `stage_dependencies(stage: &StageSpec) -> BTreeSet<String>`.
- Add `order_stages(stages: Vec<StageSpec>) -> Result<Vec<StageSpec>, LlmffError>` using Kahn's algorithm.
- Preserve manifest order as the ready-stage tiebreaker by scanning the original stage vector each iteration.
- Return `GraphValidation("cycle detected in graph")` if no stage can be selected while unordered stages remain.

- [x] **Step 4: Run tests to verify GREEN**

Run:

```bash
cargo test -p llmff-core graph::tests::orders_forward graph::tests::rejects_stage_reference_cycle
```

Expected: PASS.

- [x] **Step 5: Commit**

```bash
git add crates/llmff-core/src/graph.rs
git commit -m "feat: order graph stages by dependency"
```

## Task 2: Engine Runs Out-of-Order Manifests

**Files:**
- Modify: `crates/llmff-core/src/engine.rs`
- Test: `crates/llmff-core/src/engine.rs`

- [x] **Step 1: Write failing engine test**

Add this test to `crates/llmff-core/src/engine.rs`:

```rust
#[tokio::test]
async fn runs_manifest_in_dependency_order() {
    let dir = tempdir().unwrap();
    let prompt_path = dir.path().join("question.txt");
    let output_path = dir.path().join("answer.txt");
    std::fs::write(&prompt_path, "Return an answer object").unwrap();

    let manifest = Manifest::from_yaml_str(&format!(
        r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: write_answer
    op: write
    from: draft
    path: {}
  - id: draft
    op: infer
    from: load_prompt
    model: mock:good
  - id: load_prompt
    op: load
    input: prompt
"#,
        prompt_path.display(),
        output_path.display()
    ))
    .unwrap();

    let engine = Engine::new().with_backend(
        "mock:good",
        Arc::new(MockBackend::new("mock:good", "dependency ordered")),
    );

    let report = engine.run_manifest(manifest, dir.path()).await.unwrap();

    assert_eq!(report.final_status, RunStatus::Succeeded);
    assert_eq!(
        std::fs::read_to_string(output_path).unwrap(),
        "dependency ordered"
    );
}
```

- [x] **Step 2: Run test to verify RED if Task 1 is not yet enough**

Run:

```bash
cargo test -p llmff-core engine::tests::runs_manifest_in_dependency_order
```

Expected before Task 1 implementation: FAIL. If Task 1 already made it pass, keep the test because it proves engine integration.

- [x] **Step 3: Implement only if needed**

If the test fails after Task 1, update `run_manifest_with_options` to keep using the sorted `Graph::stages()` order and avoid iterating `manifest.graph` directly.

- [x] **Step 4: Run engine test**

Run:

```bash
cargo test -p llmff-core engine::tests::runs_manifest_in_dependency_order
```

Expected: PASS.

- [x] **Step 5: Commit**

```bash
git add crates/llmff-core/src/engine.rs
git commit -m "test: prove dependency-order execution"
```

## Task 3: Docs and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/superpowers/specs/2026-05-21-route-stage-design.md`
- Modify: `docs/superpowers/specs/2026-05-22-dependency-order-execution-design.md`
- Modify: `docs/superpowers/plans/2026-05-22-dependency-order-execution.md`

- [x] **Step 1: Update documentation**

Update README:

- Add a sentence near manifest examples: "Manifest stages may be written in any order; `llmff` executes them by dependency order."
- Change the limitation from "Pipeline execution is sequential" to "Pipeline execution is sequential after dependency ordering; parallel scheduling is not implemented yet."

Update the route stage design note to say the earlier-order target restriction was superseded by dependency-order execution.

- [x] **Step 2: Run final verification**

Run:

```bash
cargo fmt --all --check
cargo test --workspace
cargo run -p llmff -- inspect examples/json-repair.yaml
```

Expected: all commands exit 0; inspect prints `ok`.

- [ ] **Step 3: Commit**

```bash
git add README.md docs/superpowers/specs/2026-05-21-route-stage-design.md docs/superpowers/specs/2026-05-22-dependency-order-execution-design.md docs/superpowers/plans/2026-05-22-dependency-order-execution.md
git commit -m "docs: document dependency-order execution"
```

## Self-Review

- Spec coverage: dependency-order execution, forward references, route targets, cycle rejection, deterministic order, docs, and verification are covered.
- Placeholder scan: no placeholders or open-ended tasks.
- Type consistency: uses existing `Graph`, `StageSpec`, `Manifest`, `Engine`, `RunReport`, and `MockBackend` names.
