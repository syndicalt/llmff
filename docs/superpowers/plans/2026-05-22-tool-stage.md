# Tool Stage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `tool` stage that calls explicit command or HTTP tools inside an `llmff` pipeline.

**Architecture:** Extend `StageSpec` with transport fields, validate tool configuration during graph construction, and execute tools from the existing async engine dispatch. Commands use `std::process::Command` with argv and stdin/stdout, while HTTP uses the existing `reqwest` dependency.

**Tech Stack:** Rust, Tokio tests, reqwest, wiremock, tempfile.

---

## File Structure

- Modify `crates/llmff-core/src/manifest.rs` for `StageSpec` fields and parsing tests.
- Modify `crates/llmff-core/src/graph.rs` for tool transport validation.
- Modify `crates/llmff-core/src/engine.rs` for command and HTTP execution tests and implementation.
- Modify `README.md` for user-facing `tool` stage examples.

### Task 1: Parse Tool Fields

**Files:**
- Modify: `crates/llmff-core/src/manifest.rs`

- [ ] **Step 1: Write the failing test**

Add this test in `manifest.rs`:

```rust
#[test]
fn parses_tool_fields() {
    let yaml = r#"
version: 1
graph:
  - id: call_tool
    op: tool
    from: render_prompt
    command: ["/bin/cat"]
    method: POST
    url: http://127.0.0.1:8080/process
    headers:
      content-type: application/json
"#;

    let manifest = Manifest::from_yaml_str(yaml).expect("manifest should parse");
    let stage = &manifest.graph[0];

    assert_eq!(stage.command.as_deref(), Some(&["/bin/cat".to_string()][..]));
    assert_eq!(stage.method.as_deref(), Some("POST"));
    assert_eq!(stage.url.as_deref(), Some("http://127.0.0.1:8080/process"));
    assert_eq!(stage.headers["content-type"], "application/json");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p llmff-core manifest::tests::parses_tool_fields`

Expected: compile failure because `StageSpec` has no `command`, `method`, `url`, or `headers` fields.

- [ ] **Step 3: Write minimal implementation**

Add fields to `StageSpec`:

```rust
pub command: Option<Vec<String>>,
pub method: Option<String>,
pub url: Option<String>,
#[serde(default)]
pub headers: BTreeMap<String, String>,
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p llmff-core manifest::tests::parses_tool_fields`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/llmff-core/src/manifest.rs
git commit -m "feat: parse tool stage fields"
```

### Task 2: Validate Tool Configuration

**Files:**
- Modify: `crates/llmff-core/src/graph.rs`

- [ ] **Step 1: Write failing tests**

Add tests that reject a tool with neither `command` nor `url`, reject a tool with both, and reject an empty command array.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p llmff-core graph::tests::rejects_tool`

Expected: FAIL because graph validation accepts those manifests.

- [ ] **Step 3: Write minimal implementation**

Add `validate_tool_stage(stage)?` inside the graph loop. It should only apply to `op: tool`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p llmff-core graph::tests::rejects_tool`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/llmff-core/src/graph.rs
git commit -m "feat: validate tool stage configuration"
```

### Task 3: Execute Command Tools

**Files:**
- Modify: `crates/llmff-core/src/engine.rs`

- [ ] **Step 1: Write failing tests**

Add tests proving `/bin/cat` receives parent text on stdin and that `/bin/false` returns a stage execution error.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p llmff-core engine::tests::tool_stage`

Expected: FAIL because `tool` is an unknown stage.

- [ ] **Step 3: Write minimal implementation**

Add `"tool" => self.execute_tool(stage, statuses, cwd).await` and implement command execution with `std::process::Command`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p llmff-core engine::tests::tool_stage`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/llmff-core/src/engine.rs
git commit -m "feat: execute command tool stages"
```

### Task 4: Execute HTTP Tools

**Files:**
- Modify: `crates/llmff-core/src/engine.rs`

- [ ] **Step 1: Write failing test**

Add a `wiremock` test proving a POST tool sends the parent text body and captures the response body.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p llmff-core engine::tests::tool_stage_posts_parent_body_to_http_endpoint`

Expected: FAIL because HTTP tool execution is not implemented.

- [ ] **Step 3: Write minimal implementation**

Use `reqwest::Client` to send the configured method, URL, headers, and body.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p llmff-core engine::tests::tool_stage_posts_parent_body_to_http_endpoint`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/llmff-core/src/engine.rs
git commit -m "feat: execute http tool stages"
```

### Task 5: Document And Verify

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Document command and HTTP tool examples**

Add a `tool` section showing argv command usage and explicit HTTP usage.

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
git add README.md docs/superpowers/specs/2026-05-22-tool-stage-design.md docs/superpowers/plans/2026-05-22-tool-stage.md
git commit -m "docs: document tool stage"
```
