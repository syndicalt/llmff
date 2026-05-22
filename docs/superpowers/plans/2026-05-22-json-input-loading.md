# JSON Input Loading Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add explicit JSON input loading so `load` can produce typed `Value::Json` from manifest inputs.

**Architecture:** Extend `InputSpec` with an optional `format`, validate supported formats before graph execution, and decode `load` output according to the referenced input. Thread the manifest into stage type inference so dry-run validation can distinguish text and JSON loads.

**Tech Stack:** Rust workspace, `serde`, `serde_json`, existing `Manifest`, `Engine`, `Graph`, and CLI integration tests.

---

## File Structure

- Modify `crates/llmff-core/src/manifest.rs`: add `InputSpec.format` and parsing test.
- Modify `crates/llmff-core/src/engine.rs`: validate input formats, decode JSON loads, and infer load value kind from manifest input format.
- Modify `crates/llmff-cli/tests/cli_run.rs`: add CLI run/inspect/error coverage.
- Modify `README.md`: document input `format`.
- Create/modify docs under `docs/superpowers/specs` and `docs/superpowers/plans` for this slice.

## Task 1: Parse and Validate Input Formats

**Files:**
- Modify: `crates/llmff-core/src/manifest.rs`
- Modify: `crates/llmff-core/src/engine.rs`
- Modify: `crates/llmff-cli/tests/cli_run.rs`

- [x] **Step 1: Write failing manifest parsing test**

Add this test to `crates/llmff-core/src/manifest.rs`:

```rust
#[test]
fn parses_input_format() {
    let yaml = r#"
version: 1
inputs:
  payload:
    path: ./payload.json
    format: json
graph:
  - id: load_payload
    op: load
    input: payload
"#;

    let manifest = Manifest::from_yaml_str(yaml).expect("manifest should parse");

    assert_eq!(manifest.inputs["payload"].format.as_deref(), Some("json"));
}
```

- [x] **Step 2: Run test to verify RED**

Run:

```bash
cargo test -p llmff-core manifest::tests::parses_input_format
```

Expected: FAIL because `InputSpec` has no `format` field.

- [x] **Step 3: Implement minimal manifest field**

In `crates/llmff-core/src/manifest.rs`, change `InputSpec` to:

```rust
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct InputSpec {
    pub path: Option<String>,
    pub format: Option<String>,
}
```

- [x] **Step 4: Run test to verify GREEN**

Run:

```bash
cargo test -p llmff-core manifest::tests::parses_input_format
```

Expected: PASS.

- [x] **Step 5: Write failing validation tests**

Add this core test to `crates/llmff-core/src/engine.rs`:

```rust
#[test]
fn validate_manifest_rejects_unknown_input_format() {
    let manifest = Manifest::from_yaml_str(
        r#"
version: 1
inputs:
  payload:
    path: payload.json
    format: yaml
graph:
  - id: load_payload
    op: load
    input: payload
"#,
    )
    .unwrap();

    let error = Engine::new()
        .validate_manifest(manifest)
        .expect_err("unknown input format should be rejected");

    assert!(error
        .to_string()
        .contains("input `payload` has unsupported format `yaml`"));
}
```

Add this CLI test to `crates/llmff-cli/tests/cli_run.rs`:

```rust
#[test]
fn inspect_rejects_unknown_input_format() {
    let dir = tempfile::tempdir().unwrap();
    let payload = dir.path().join("payload.json");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&payload, r#"{"kind":"simple"}"#).unwrap();
    std::fs::write(
        &manifest,
        format!(
            r#"
version: 1
inputs:
  payload:
    path: {}
    format: yaml
graph:
  - id: load_payload
    op: load
    input: payload
"#,
            payload.display()
        ),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("llmff").unwrap();
    cmd.args(["inspect", manifest.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "input `payload` has unsupported format `yaml`",
        ));
}
```

- [x] **Step 6: Run validation tests to verify RED**

Run:

```bash
cargo test -p llmff-core validate_manifest_rejects_unknown_input_format
cargo test -p llmff --test cli_run inspect_rejects_unknown_input_format
```

Expected: FAIL because unsupported input formats are currently accepted.

- [x] **Step 7: Implement input format validation**

In `Engine::validate_manifest`, validate inputs before building the graph:

```rust
validate_input_formats(&manifest)?;
let graph = Graph::from_manifest(manifest)?;
```

Add helper functions in `crates/llmff-core/src/engine.rs`:

```rust
fn validate_input_formats(manifest: &Manifest) -> Result<(), LlmffError> {
    for (id, input) in &manifest.inputs {
        match input_format(input.format.as_deref()) {
            Some(_) => {}
            None => {
                let format = input.format.as_deref().unwrap_or_default();
                return Err(LlmffError::GraphValidation(format!(
                    "input `{id}` has unsupported format `{format}`"
                )));
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputFormat {
    Text,
    Json,
}

fn input_format(format: Option<&str>) -> Option<InputFormat> {
    match format.unwrap_or("text") {
        "text" => Some(InputFormat::Text),
        "json" => Some(InputFormat::Json),
        _ => None,
    }
}
```

- [x] **Step 8: Run validation tests to verify GREEN and commit**

Run:

```bash
cargo test -p llmff-core manifest::tests::parses_input_format validate_manifest_rejects_unknown_input_format
cargo test -p llmff --test cli_run inspect_rejects_unknown_input_format
```

Expected: PASS.

Commit:

```bash
git add crates/llmff-core/src/manifest.rs crates/llmff-core/src/engine.rs crates/llmff-cli/tests/cli_run.rs
git commit -m "feat: validate input formats"
```

## Task 2: Execute JSON Loads

**Files:**
- Modify: `crates/llmff-core/src/engine.rs`
- Modify: `crates/llmff-cli/tests/cli_run.rs`

- [x] **Step 1: Write failing runtime test**

Add this test to `crates/llmff-core/src/engine.rs`:

```rust
#[tokio::test]
async fn load_stage_reads_json_input_format() {
    let dir = tempdir().unwrap();
    let payload_path = dir.path().join("payload.json");
    let output_path = dir.path().join("selected.txt");
    std::fs::write(&payload_path, r#"{"kind":"simple","answer":"ok"}"#).unwrap();

    let manifest = Manifest::from_yaml_str(&format!(
        r#"
version: 1
inputs:
  payload:
    path: {}
    format: json
graph:
  - id: load_payload
    op: load
    input: payload
  - id: simple_answer
    op: template
    from: load_payload
    path: simple.tmpl
  - id: choose
    op: route
    from: load_payload
    field: kind
    cases:
      simple: simple_answer
outputs:
  final:
    from: choose
    path: {}
"#,
        payload_path.display(),
        output_path.display()
    ))
    .unwrap();
    std::fs::write(dir.path().join("simple.tmpl"), "{{answer}}").unwrap();

    let report = Engine::new().run_manifest(manifest, dir.path()).await.unwrap();

    assert_eq!(report.final_status, RunStatus::Succeeded);
    assert_eq!(std::fs::read_to_string(output_path).unwrap(), "ok");
}
```

- [x] **Step 2: Run runtime test to verify RED**

Run:

```bash
cargo test -p llmff-core load_stage_reads_json_input_format
```

Expected: FAIL because `load` returns text and field routing requires JSON.

- [x] **Step 3: Implement JSON decoding for load**

In `execute_load`, after reading input text, decode by input format:

```rust
decode_input(stage, input_name, input.format.as_deref(), text)
```

Add:

```rust
fn decode_input(
    stage: &StageSpec,
    input_name: &str,
    format: Option<&str>,
    source: String,
) -> Result<StageStatus, LlmffError> {
    match input_format(format).ok_or_else(|| LlmffError::StageExecution {
        stage_id: stage.id.clone(),
        message: format!(
            "input `{input_name}` has unsupported format `{}`",
            format.unwrap_or_default()
        ),
    })? {
        InputFormat::Text => Ok(StageStatus::Success(Value::Text(source))),
        InputFormat::Json => serde_json::from_str(&source)
            .map(Value::Json)
            .map(StageStatus::Success)
            .map_err(|error| LlmffError::StageExecution {
                stage_id: stage.id.clone(),
                message: format!("input `{input_name}` is not valid JSON: {error}"),
            }),
    }
}
```

- [x] **Step 4: Run runtime test to verify GREEN**

Run:

```bash
cargo test -p llmff-core load_stage_reads_json_input_format
```

Expected: PASS.

- [x] **Step 5: Add invalid JSON CLI test**

Add this test to `crates/llmff-cli/tests/cli_run.rs`:

```rust
#[test]
fn run_reports_invalid_json_input_format() {
    let dir = tempfile::tempdir().unwrap();
    let payload = dir.path().join("payload.json");
    let output = dir.path().join("selected.txt");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&payload, "{not-json").unwrap();
    std::fs::write(
        &manifest,
        format!(
            r#"
version: 1
inputs:
  payload:
    path: {}
    format: json
graph:
  - id: load_payload
    op: load
    input: payload
outputs:
  final:
    from: load_payload
    path: {}
"#,
            payload.display(),
            output.display()
        ),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("llmff").unwrap();
    cmd.args(["run", manifest.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("input `payload` is not valid JSON"));
}
```

Run:

```bash
cargo test -p llmff --test cli_run run_reports_invalid_json_input_format
```

Expected: PASS after JSON decoding is implemented.

- [x] **Step 6: Commit**

```bash
git add crates/llmff-core/src/engine.rs crates/llmff-cli/tests/cli_run.rs
git commit -m "feat: load json inputs"
```

## Task 3: Static Type Inference and Documentation

**Files:**
- Modify: `crates/llmff-core/src/engine.rs`
- Modify: `crates/llmff-cli/tests/cli_run.rs`
- Modify: `README.md`
- Modify: `docs/superpowers/plans/2026-05-22-json-input-loading.md`

- [x] **Step 1: Write failing inspect test for JSON field route**

Add this CLI test to `crates/llmff-cli/tests/cli_run.rs`:

```rust
#[test]
fn inspect_accepts_field_route_from_json_input() {
    let dir = tempfile::tempdir().unwrap();
    let payload = dir.path().join("payload.json");
    let template = dir.path().join("simple.tmpl");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&payload, r#"{"kind":"simple","answer":"ok"}"#).unwrap();
    std::fs::write(&template, "{{answer}}").unwrap();
    std::fs::write(
        &manifest,
        format!(
            r#"
version: 1
inputs:
  payload:
    path: {}
    format: json
graph:
  - id: load_payload
    op: load
    input: payload
  - id: simple_answer
    op: template
    from: load_payload
    path: {}
  - id: choose
    op: route
    from: load_payload
    field: kind
    cases:
      simple: simple_answer
"#,
            payload.display(),
            template.display()
        ),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("llmff").unwrap();
    cmd.args(["inspect", manifest.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("ok"));
}
```

- [x] **Step 2: Run inspect test to verify RED**

Run:

```bash
cargo test -p llmff --test cli_run inspect_accepts_field_route_from_json_input
```

Expected: FAIL because static type validation still treats all load stages as text.

- [x] **Step 3: Implement load kind inference from manifest**

Change `Engine::validate_manifest` to call:

```rust
validate_stage_types(&graph, &manifest)?;
```

Change `validate_stage_types` and `infer_stage_value_kind` signatures to accept the manifest:

```rust
fn validate_stage_types(graph: &Graph, manifest: &Manifest) -> Result<(), LlmffError>
fn infer_stage_value_kind(
    stage: &StageSpec,
    manifest: &Manifest,
    previous: &BTreeMap<String, StageValueKind>,
) -> StageValueKind
```

For `load`, infer from the referenced input:

```rust
"load" => stage
    .input
    .as_ref()
    .and_then(|input_id| manifest.inputs.get(input_id))
    .and_then(|input| input_format(input.format.as_deref()))
    .map(|format| match format {
        InputFormat::Text => StageValueKind::Text,
        InputFormat::Json => StageValueKind::Json,
    })
    .unwrap_or(StageValueKind::Text),
```

- [x] **Step 4: Run inspect test to verify GREEN**

Run:

```bash
cargo test -p llmff --test cli_run inspect_accepts_field_route_from_json_input
```

Expected: PASS.

- [x] **Step 5: Document input formats**

Update `README.md` near file-backed resources:

```markdown
Inputs default to text. Set `format: json` to parse an input into a structured JSON value:

```yaml
inputs:
  payload:
    path: ./payload.json
    format: json
```

JSON inputs can be templated by object field and used by field-based routes. Invalid JSON fails the load stage with a stage execution error.
```

- [x] **Step 6: Run final verification**

Run:

```bash
cargo fmt --all --check
cargo test --workspace
cargo run -p llmff -- inspect examples/json-repair.yaml
```

Expected: all commands exit 0; inspect prints `ok`.

- [x] **Step 7: Commit**

```bash
git add README.md crates/llmff-core/src/engine.rs crates/llmff-cli/tests/cli_run.rs docs/superpowers/plans/2026-05-22-json-input-loading.md
git commit -m "docs: document json input loading"
```

## Self-Review

- Spec coverage: parsing, validation, runtime loading, static type inference, CLI behavior, docs, and verification are covered.
- Placeholder scan: no placeholders or open-ended implementation steps remain.
- Type consistency: uses existing `InputSpec`, `Manifest`, `Engine`, `StageSpec`, `Value::Json`, and `StageValueKind` names.
