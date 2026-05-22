# Sampling Parameters Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add common sampling controls to model-calling stages and backend requests.

**Architecture:** Extend `StageSpec` and `InferRequest` with optional `top_p` and `max_tokens`, validate stage values in `Engine::validate_stage`, and map request values in each backend adapter. Keep provider-specific translation contained in backend modules.

**Tech Stack:** Rust workspace, `serde`, `serde_json`, `clap` CLI, existing core backend tests, engine tests, and CLI tests.

---

## File Structure

- Modify `crates/llmff-core/src/manifest.rs`: parse `top_p` and `max_tokens`.
- Modify `crates/llmff-core/src/backend.rs`: extend `InferRequest`; map sampling params into OpenAI-compatible and Ollama request bodies; add tests.
- Modify `crates/llmff-core/src/engine.rs`: validate sampling params and pass them for `infer` and `repair`.
- Modify `crates/llmff-core/src/inline_graph.rs`: parse inline `top_p` and `max_tokens`.
- Modify `crates/llmff-cli/tests/cli_run.rs`: add inspect validation coverage.
- Modify `README.md`: document sampling controls.

## Task 1: Parse and Validate Sampling Fields

**Files:**
- Modify: `crates/llmff-core/src/manifest.rs`
- Modify: `crates/llmff-core/src/engine.rs`
- Modify: `crates/llmff-cli/tests/cli_run.rs`

- [x] **Step 1: Write failing manifest parsing test**

Add this test to `crates/llmff-core/src/manifest.rs`:

```rust
#[test]
fn parses_sampling_fields() {
    let yaml = r#"
version: 1
graph:
  - id: draft
    op: infer
    from: prompt
    model: mock:good
    temperature: 0.2
    top_p: 0.9
    max_tokens: 256
"#;

    let manifest = Manifest::from_yaml_str(yaml).expect("manifest should parse");
    let stage = &manifest.graph[0];

    assert_eq!(stage.temperature, Some(0.2));
    assert_eq!(stage.top_p, Some(0.9));
    assert_eq!(stage.max_tokens, Some(256));
}
```

- [x] **Step 2: Run parsing test to verify RED**

Run:

```bash
cargo test -p llmff-core manifest::tests::parses_sampling_fields
```

Expected: FAIL because `StageSpec` does not have `top_p` or `max_tokens`.

- [x] **Step 3: Implement manifest fields**

Add fields to `StageSpec` in `crates/llmff-core/src/manifest.rs`:

```rust
pub top_p: Option<f32>,
pub max_tokens: Option<u32>,
```

Add the same fields with `None` defaults in `empty_stage` in `crates/llmff-core/src/inline_graph.rs`.

- [x] **Step 4: Run parsing test to verify GREEN**

Run:

```bash
cargo test -p llmff-core manifest::tests::parses_sampling_fields
```

Expected: PASS.

- [x] **Step 5: Write failing validation tests**

Add this core test to `crates/llmff-core/src/engine.rs`:

```rust
#[test]
fn validate_manifest_rejects_invalid_sampling_parameters() {
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
    model: mock:good
    top_p: 1.5
"#,
    )
    .unwrap();

    let error = Engine::new()
        .with_backend("mock:good", Arc::new(MockBackend::new("mock:good", "ok")))
        .validate_manifest(manifest)
        .expect_err("invalid sampling parameter should be rejected");

    assert!(error
        .to_string()
        .contains("stage `draft` failed: top_p must be between 0 and 1"));
}
```

Add this CLI test to `crates/llmff-cli/tests/cli_run.rs`:

```rust
#[test]
fn inspect_rejects_invalid_sampling_parameters() {
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
    model: mock:good
    max_tokens: 0
"#,
            prompt.display()
        ),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("llmff").unwrap();
    cmd.args(["inspect", manifest.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("max_tokens must be greater than 0"));
}
```

- [x] **Step 6: Run validation tests to verify RED**

Run:

```bash
cargo test -p llmff-core validate_manifest_rejects_invalid_sampling_parameters
cargo test -p llmff --test cli_run inspect_rejects_invalid_sampling_parameters
```

Expected: FAIL because invalid sampling parameters are accepted.

- [x] **Step 7: Implement stage validation**

In `Engine::validate_stage`, call this helper before matching operations:

```rust
validate_sampling_parameters(stage)?;
```

Add:

```rust
fn validate_sampling_parameters(stage: &StageSpec) -> Result<(), LlmffError> {
    if let Some(temperature) = stage.temperature {
        if temperature < 0.0 {
            return Err(stage_validation_error(stage, "temperature must be greater than or equal to 0"));
        }
    }
    if let Some(top_p) = stage.top_p {
        if !(0.0..=1.0).contains(&top_p) {
            return Err(stage_validation_error(stage, "top_p must be between 0 and 1"));
        }
    }
    if let Some(0) = stage.max_tokens {
        return Err(stage_validation_error(stage, "max_tokens must be greater than 0"));
    }
    Ok(())
}
```

- [x] **Step 8: Run validation tests to verify GREEN and commit**

Run:

```bash
cargo test -p llmff-core manifest::tests::parses_sampling_fields validate_manifest_rejects_invalid_sampling_parameters
cargo test -p llmff --test cli_run inspect_rejects_invalid_sampling_parameters
```

Expected: PASS.

Commit:

```bash
git add crates/llmff-core/src/manifest.rs crates/llmff-core/src/inline_graph.rs crates/llmff-core/src/engine.rs crates/llmff-cli/tests/cli_run.rs
git commit -m "feat: validate sampling parameters"
```

## Task 2: Propagate Sampling Into Backend Requests

**Files:**
- Modify: `crates/llmff-core/src/backend.rs`
- Modify: `crates/llmff-core/src/engine.rs`

- [x] **Step 1: Write failing backend tests**

Update `openai_compatible_backend_reads_chat_completion_content` in `crates/llmff-core/src/backend.rs` so the request includes:

```rust
top_p: Some(0.9),
max_tokens: Some(256),
```

and add assertions:

```rust
assert_eq!(body["top_p"], 0.9);
assert_eq!(body["max_tokens"], 256);
```

Update `ollama_backend_reads_chat_message_content` so the request includes:

```rust
top_p: Some(0.8),
max_tokens: Some(128),
```

and add assertions:

```rust
assert_eq!(body["options"]["top_p"], 0.8);
assert_eq!(body["options"]["num_predict"], 128);
```

- [x] **Step 2: Run backend tests to verify RED**

Run:

```bash
cargo test -p llmff-core backend::tests::openai_compatible_backend_reads_chat_completion_content backend::tests::ollama_backend_reads_chat_message_content
```

Expected: FAIL because `InferRequest` does not carry these fields.

- [x] **Step 3: Extend `InferRequest`**

In `crates/llmff-core/src/backend.rs`, add:

```rust
pub top_p: Option<f32>,
pub max_tokens: Option<u32>,
```

Update all `InferRequest` construction sites in tests and engine code with the new fields.

- [x] **Step 4: Map OpenAI-compatible request body**

Replace the current fixed JSON construction with a mutable object:

```rust
let mut body = json!({
    "model": request.model,
    "messages": [
        {
            "role": "user",
            "content": request.prompt
        }
    ],
});
if let Some(temperature) = request.temperature {
    body["temperature"] = json!(temperature);
}
if let Some(top_p) = request.top_p {
    body["top_p"] = json!(top_p);
}
if let Some(max_tokens) = request.max_tokens {
    body["max_tokens"] = json!(max_tokens);
}
```

- [x] **Step 5: Map Ollama request body**

In `ollama_chat_request_body`, build an `options` object with present values:

```rust
let mut options = serde_json::Map::new();
if let Some(temperature) = request.temperature {
    options.insert("temperature".to_string(), json!(temperature));
}
if let Some(top_p) = request.top_p {
    options.insert("top_p".to_string(), json!(top_p));
}
if let Some(max_tokens) = request.max_tokens {
    options.insert("num_predict".to_string(), json!(max_tokens));
}
if !options.is_empty() {
    body["options"] = serde_json::Value::Object(options);
}
```

- [x] **Step 6: Pass stage params from engine**

In `execute_infer` and `execute_repair`, add:

```rust
top_p: stage.top_p,
max_tokens: stage.max_tokens,
```

- [x] **Step 7: Run backend tests to verify GREEN and commit**

Run:

```bash
cargo test -p llmff-core backend::tests::openai_compatible_backend_reads_chat_completion_content
cargo test -p llmff-core backend::tests::ollama_backend_reads_chat_message_content
```

Expected: PASS.

Commit:

```bash
git add crates/llmff-core/src/backend.rs crates/llmff-core/src/engine.rs
git commit -m "feat: forward sampling parameters"
```

## Task 3: Inline Graph and Documentation

**Files:**
- Modify: `crates/llmff-core/src/inline_graph.rs`
- Modify: `README.md`
- Modify: `docs/superpowers/plans/2026-05-22-sampling-params.md`

- [x] **Step 1: Write failing inline graph test**

Add assertions to `parses_linear_inline_graph`:

```rust
assert_eq!(manifest.graph[1].top_p, Some(0.9));
assert_eq!(manifest.graph[1].max_tokens, Some(256));
```

Change the inline graph source to:

```rust
"load | infer(model=mock:good,temperature=0.2,top_p=0.9,max_tokens=256) | write(-)"
```

- [x] **Step 2: Run inline graph test to verify RED**

Run:

```bash
cargo test -p llmff-core inline_graph::tests::parses_linear_inline_graph
```

Expected: FAIL because inline graph does not parse `top_p` or `max_tokens`.

- [x] **Step 3: Implement inline parsing**

In `apply_inline_params`, add:

```rust
"top_p" => {
    stage.top_p = Some(value.parse::<f32>().map_err(|error| {
        inline_graph_error(format!("invalid top_p `{value}`: {error}"))
    })?)
}
"max_tokens" => {
    stage.max_tokens = Some(value.parse::<u32>().map_err(|error| {
        inline_graph_error(format!("invalid max_tokens `{value}`: {error}"))
    })?)
}
```

- [x] **Step 4: Run inline graph test to verify GREEN**

Run:

```bash
cargo test -p llmff-core inline_graph::tests::parses_linear_inline_graph
```

Expected: PASS.

- [x] **Step 5: Document sampling controls**

Update `README.md` near backend notes:

```markdown
Model-calling stages accept portable sampling controls:

```yaml
graph:
  - id: draft
    op: infer
    from: load_prompt
    model: openai:gpt-test
    temperature: 0.2
    top_p: 0.9
    max_tokens: 256
```

OpenAI-compatible backends receive `temperature`, `top_p`, and `max_tokens`. Ollama receives the same controls under `options`, with `max_tokens` mapped to `num_predict`.
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
git add README.md crates/llmff-core/src/inline_graph.rs docs/superpowers/plans/2026-05-22-sampling-params.md
git commit -m "docs: document sampling parameters"
```

## Self-Review

- Spec coverage: manifest parsing, validation, backend propagation, inline graph parsing, docs, and verification are covered.
- Placeholder scan: no placeholders or open-ended implementation steps remain.
- Type consistency: uses existing `StageSpec`, `InferRequest`, `Engine`, `OpenAiCompatibleBackend`, and `OllamaBackend` names.
