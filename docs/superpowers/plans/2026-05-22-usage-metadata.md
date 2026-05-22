# Usage Metadata Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Capture provider token usage and expose it through traces and trace summaries.

**Architecture:** Add `UsageMetadata` to backend responses, parse provider-specific usage in backend adapters, store usage by stage id during engine execution, and write optional usage fields into `stage_finished` trace events. Keep usage metadata optional and additive.

**Tech Stack:** Rust workspace, `serde`, `serde_json`, existing backend tests with `wiremock`, engine trace tests, and CLI trace tests.

---

## File Structure

- Modify `crates/llmff-core/src/backend.rs`: add `UsageMetadata`, extend `InferResponse`, parse OpenAI-compatible and Ollama usage.
- Modify `crates/llmff-core/src/trace.rs`: add optional usage fields to trace events.
- Modify `crates/llmff-core/src/engine.rs`: retain model response usage per stage and add it to trace metadata.
- Modify `crates/llmff-cli/src/commands.rs`: show usage in trace summaries.
- Modify `crates/llmff-cli/tests/cli_run.rs`: add trace summary coverage.
- Modify `README.md`: document usage metadata.

## Task 1: Parse Backend Usage

**Files:**
- Modify: `crates/llmff-core/src/backend.rs`

- [x] **Step 1: Write failing OpenAI-compatible usage test**

In `openai_compatible_backend_reads_chat_completion_content`, add this `usage` object to the mocked JSON response:

```rust
"usage": {
    "prompt_tokens": 12,
    "completion_tokens": 8,
    "total_tokens": 20
}
```

Add assertions after response assertions:

```rust
let usage = response.usage.expect("usage should be parsed");
assert_eq!(usage.prompt_tokens, Some(12));
assert_eq!(usage.completion_tokens, Some(8));
assert_eq!(usage.total_tokens, Some(20));
```

- [x] **Step 2: Run OpenAI-compatible test to verify RED**

Run:

```bash
cargo test -p llmff-core backend::tests::openai_compatible_backend_reads_chat_completion_content
```

Expected: FAIL because `InferResponse` has no `usage`.

- [x] **Step 3: Implement OpenAI-compatible usage parsing**

Add:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageMetadata {
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
}
```

Extend `InferResponse`:

```rust
pub usage: Option<UsageMetadata>,
```

Extend `ChatCompletionResponse`:

```rust
usage: Option<OpenAiUsage>,
```

Add:

```rust
#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
    total_tokens: Option<u64>,
}

impl From<OpenAiUsage> for UsageMetadata {
    fn from(usage: OpenAiUsage) -> Self {
        Self {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
        }
    }
}
```

Set `InferResponse.usage` in every backend response. Mock backends use `None`.

- [x] **Step 4: Run OpenAI-compatible test to verify GREEN**

Run:

```bash
cargo test -p llmff-core backend::tests::openai_compatible_backend_reads_chat_completion_content
```

Expected: PASS.

- [x] **Step 5: Write failing Ollama usage test**

In `ollama_backend_reads_chat_message_content`, add these response fields:

```rust
"prompt_eval_count": 7,
"eval_count": 5,
```

Add assertions:

```rust
let usage = response.usage.expect("usage should be parsed");
assert_eq!(usage.prompt_tokens, Some(7));
assert_eq!(usage.completion_tokens, Some(5));
assert_eq!(usage.total_tokens, Some(12));
```

- [x] **Step 6: Run Ollama test to verify RED**

Run:

```bash
cargo test -p llmff-core backend::tests::ollama_backend_reads_chat_message_content
```

Expected: FAIL because Ollama usage is not parsed yet.

- [x] **Step 7: Implement Ollama usage parsing**

Extend `OllamaChatResponse`:

```rust
prompt_eval_count: Option<u64>,
eval_count: Option<u64>,
```

Add:

```rust
fn ollama_usage(response: &OllamaChatResponse) -> Option<UsageMetadata> {
    if response.prompt_eval_count.is_none() && response.eval_count.is_none() {
        return None;
    }
    Some(UsageMetadata {
        prompt_tokens: response.prompt_eval_count,
        completion_tokens: response.eval_count,
        total_tokens: match (response.prompt_eval_count, response.eval_count) {
            (Some(prompt), Some(completion)) => Some(prompt + completion),
            _ => None,
        },
    })
}
```

Use it in `OllamaBackend::infer`.

- [x] **Step 8: Run backend tests to verify GREEN and commit**

Run:

```bash
cargo test -p llmff-core backend::tests::openai_compatible_backend_reads_chat_completion_content
cargo test -p llmff-core backend::tests::ollama_backend_reads_chat_message_content
```

Expected: PASS.

Commit:

```bash
git add crates/llmff-core/src/backend.rs
git commit -m "feat: parse backend usage metadata"
```

## Task 2: Add Usage to Trace Events

**Files:**
- Modify: `crates/llmff-core/src/trace.rs`
- Modify: `crates/llmff-core/src/engine.rs`

- [x] **Step 1: Write failing trace test**

Add this helper backend inside `engine.rs` tests:

```rust
#[derive(Debug)]
struct UsageBackend {
    model: String,
    text: String,
    usage: UsageMetadata,
}

#[async_trait::async_trait]
impl Backend for UsageBackend {
    async fn infer(&self, request: InferRequest) -> Result<InferResponse, LlmffError> {
        assert_eq!(request.model, self.model);
        Ok(InferResponse {
            model: request.model,
            text: self.text.clone(),
            usage: Some(self.usage.clone()),
        })
    }
}
```

Add test:

```rust
#[tokio::test]
async fn trace_events_include_model_usage_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let prompt_path = dir.path().join("question.txt");
    let output_path = dir.path().join("answer.txt");
    let trace_path = dir.path().join("trace.jsonl");
    std::fs::write(&prompt_path, "hello").unwrap();

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
  - id: draft
    op: infer
    from: load_prompt
    model: usage:test-model
outputs:
  final:
    from: draft
    path: {}
"#,
        prompt_path.display(),
        output_path.display()
    ))
    .unwrap();
    let engine = Engine::new().with_backend(
        "usage",
        Arc::new(UsageBackend {
            model: "test-model".to_string(),
            text: "ok".to_string(),
            usage: UsageMetadata {
                prompt_tokens: Some(12),
                completion_tokens: Some(8),
                total_tokens: Some(20),
            },
        }),
    );

    engine
        .run_manifest_with_options(
            manifest,
            dir.path(),
            RunOptions {
                run_id: "trace-test".to_string(),
                trace_path: Some(trace_path.clone()),
            },
        )
        .await
        .unwrap();

    let trace = std::fs::read_to_string(trace_path).unwrap();
    let events = parse_trace_events(&trace);
    let draft_finished = trace_stage_finished(&events, "draft");

    assert_eq!(draft_finished["prompt_tokens"], 12);
    assert_eq!(draft_finished["completion_tokens"], 8);
    assert_eq!(draft_finished["total_tokens"], 20);
}
```

- [x] **Step 2: Run trace test to verify RED**

Run:

```bash
cargo test -p llmff-core trace_events_include_model_usage_metadata
```

Expected: FAIL because trace events do not include usage fields.

- [x] **Step 3: Add trace fields and engine usage tracking**

In `TraceEvent`, add optional fields:

```rust
#[serde(skip_serializing_if = "Option::is_none")]
pub prompt_tokens: Option<u64>,
#[serde(skip_serializing_if = "Option::is_none")]
pub completion_tokens: Option<u64>,
#[serde(skip_serializing_if = "Option::is_none")]
pub total_tokens: Option<u64>,
```

Add matching fields to `TraceMetadata`.

Change `execute_stage` to return `StageOutcome`:

```rust
struct StageOutcome {
    status: StageStatus,
    usage: Option<UsageMetadata>,
}
```

Make non-model stages return `StageOutcome::without_usage(status)` and model stages attach `response.usage`.

When writing `TraceEvent`, set the usage fields from metadata.

- [x] **Step 4: Run trace test to verify GREEN and commit**

Run:

```bash
cargo test -p llmff-core trace_events_include_model_usage_metadata
```

Expected: PASS.

Commit:

```bash
git add crates/llmff-core/src/trace.rs crates/llmff-core/src/engine.rs
git commit -m "feat: trace model usage metadata"
```

## Task 3: Trace CLI and Documentation

**Files:**
- Modify: `crates/llmff-cli/src/commands.rs`
- Modify: `crates/llmff-cli/tests/cli_run.rs`
- Modify: `README.md`
- Modify: `docs/superpowers/plans/2026-05-22-usage-metadata.md`

- [x] **Step 1: Write failing trace CLI test**

Update `trace_command_summarizes_trace_jsonl` fixture to include:

```json
"prompt_tokens":12,"completion_tokens":8,"total_tokens":20
```

Add stdout assertions:

```rust
.stdout(predicate::str::contains("usage=20"))
.stdout(predicate::str::contains("prompt_tokens=12"))
.stdout(predicate::str::contains("completion_tokens=8"))
```

- [x] **Step 2: Run trace CLI test to verify RED**

Run:

```bash
cargo test -p llmff --test cli_run trace_command_summarizes_trace_jsonl
```

Expected: FAIL because trace summaries do not print usage metadata.

- [x] **Step 3: Implement trace summary usage fields**

In `summarize_trace_event`, after provider metadata, add:

```rust
if let Some(total) = integer_field(event, "total_tokens") {
    parts.push(format!("usage={total}"));
}
if let Some(prompt) = integer_field(event, "prompt_tokens") {
    parts.push(format!("prompt_tokens={prompt}"));
}
if let Some(completion) = integer_field(event, "completion_tokens") {
    parts.push(format!("completion_tokens={completion}"));
}
```

- [x] **Step 4: Run trace CLI test to verify GREEN**

Run:

```bash
cargo test -p llmff --test cli_run trace_command_summarizes_trace_jsonl
```

Expected: PASS.

- [x] **Step 5: Document usage metadata**

Update README trace notes:

```markdown
Model stage traces include token usage when the backend reports it: `prompt_tokens`, `completion_tokens`, and `total_tokens`. `llmff trace` summarizes total usage as `usage=<total>`.
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
git add README.md crates/llmff-cli/src/commands.rs crates/llmff-cli/tests/cli_run.rs docs/superpowers/plans/2026-05-22-usage-metadata.md
git commit -m "docs: document usage metadata"
```

## Self-Review

- Spec coverage: backend usage parsing, engine trace propagation, trace CLI summary, docs, and verification are covered.
- Placeholder scan: no placeholders or vague implementation steps remain.
- Type consistency: uses `InferResponse`, `UsageMetadata`, `TraceEvent`, `TraceMetadata`, and existing trace CLI helper names.
