# Inline Graph Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `llmff run -i <path> -g '<pipeline>'` for linear inline graph execution.

**Architecture:** Add a core inline graph parser that normalizes CLI syntax into `Manifest`, then update the CLI to choose between manifest loading and inline graph construction before calling the existing engine. Keep execution semantics in core.

**Tech Stack:** Rust, clap, assert_cmd, tempfile.

---

## File Structure

- Create `crates/llmff-core/src/inline_graph.rs` for parsing and normalization.
- Modify `crates/llmff-core/src/lib.rs` to expose the module.
- Modify `crates/llmff-cli/src/commands.rs` for `run -i/--input -g/--graph`.
- Modify `crates/llmff-cli/tests/cli_run.rs` for CLI behavior.
- Modify `README.md` for the inline graph command.

### Task 1: Parse Inline Graphs In Core

**Files:**
- Create: `crates/llmff-core/src/inline_graph.rs`
- Modify: `crates/llmff-core/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Add tests proving:

```rust
let manifest = Manifest::from_inline_graph(
    "load | infer(model=mock:good,temperature=0.2) | write(-)",
    Some("question.txt".to_string()),
).unwrap();
```

Expected:

- `inputs["prompt"].path == Some("question.txt")`
- `graph[0]` is `load_1`
- `graph[1]` is `infer_2` with `from: load_1`, `model: mock:good`, `temperature: 0.2`
- `graph[2]` is `write_3` with `from: infer_2`, `path: -`

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p llmff-core inline_graph::tests`

Expected: compile failure because the module/function does not exist.

- [ ] **Step 3: Write minimal implementation**

Implement `Manifest::from_inline_graph(source, input_path) -> Result<Manifest, LlmffError>` using explicit parsing, not ad hoc execution.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p llmff-core inline_graph::tests`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/llmff-core/src/inline_graph.rs crates/llmff-core/src/lib.rs
git commit -m "feat: parse inline graph manifests"
```

### Task 2: Wire Inline Graph Into CLI Run

**Files:**
- Modify: `crates/llmff-cli/src/commands.rs`
- Modify: `crates/llmff-cli/tests/cli_run.rs`

- [ ] **Step 1: Write failing CLI tests**

Add tests proving:

```bash
llmff run -i question.txt -g 'load | infer(model=mock:good) | write(answer.json)'
```

writes `answer.json`, and:

```bash
llmff run pipeline.yaml -g 'load | write(-)'
```

fails with `provide either manifest or --graph`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p llmff --test cli_run inline_graph`

Expected: FAIL because the CLI does not accept `-i` or `-g`.

- [ ] **Step 3: Write minimal implementation**

Make the manifest positional argument optional and add:

```rust
#[arg(short = 'i', long = "input")]
input: Option<PathBuf>,
#[arg(short = 'g', long = "graph")]
graph: Option<String>,
```

Then construct either a manifest from disk or `Manifest::from_inline_graph`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p llmff --test cli_run inline_graph`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/llmff-cli/src/commands.rs crates/llmff-cli/tests/cli_run.rs
git commit -m "feat: run inline graphs from cli"
```

### Task 3: Document And Verify

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Document inline graph usage**

Add examples for:

```bash
llmff run -i prompt.txt -g 'load | infer(model=mock:good) | write(-)'
cat prompt.txt | llmff run -g 'load | infer(model=mock:good) | write(-)'
```

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
git add README.md docs/superpowers/specs/2026-05-22-inline-graph-design.md docs/superpowers/plans/2026-05-22-inline-graph.md
git commit -m "docs: document inline graph usage"
```
