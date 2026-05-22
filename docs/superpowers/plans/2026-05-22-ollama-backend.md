# Ollama Backend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a native Ollama backend adapter and CLI registration flag.

**Architecture:** Implement `OllamaBackend` in `llmff-core::backend` behind the existing `Backend` trait. Add a separate CLI flag `--ollama alias=url` so OpenAI-compatible and native Ollama registrations stay explicit.

**Tech Stack:** Rust, reqwest, wiremock, assert_cmd.

---

## File Structure

- Modify `crates/llmff-core/src/backend.rs` for the backend and unit test.
- Modify `crates/llmff-cli/src/commands.rs` for `--ollama`.
- Modify `crates/llmff-cli/tests/cli_run.rs` for CLI integration.
- Modify `README.md` for backend docs.

### Task 1: Implement Core Ollama Backend

**Files:**
- Modify: `crates/llmff-core/src/backend.rs`

- [ ] **Step 1: Write failing test**

Add a `wiremock` test proving `OllamaBackend::infer` posts to `/api/chat` with `model`, `messages`, `stream: false`, and `options.temperature`, then returns `message.content`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p llmff-core backend::tests::ollama_backend_reads_chat_message_content`

Expected: compile failure because `OllamaBackend` does not exist.

- [ ] **Step 3: Write minimal implementation**

Add `OllamaBackend`, request body construction, response parsing, and backend errors.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p llmff-core backend::tests::ollama_backend_reads_chat_message_content`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/llmff-core/src/backend.rs
git commit -m "feat: add ollama backend"
```

### Task 2: Register Ollama Backends From CLI

**Files:**
- Modify: `crates/llmff-cli/src/commands.rs`
- Modify: `crates/llmff-cli/tests/cli_run.rs`

- [ ] **Step 1: Write failing CLI tests**

Add tests proving `backends list` prints `ollama`, and `llmff run ... --ollama ollama=<url>` can run a manifest with `model: ollama:test-model`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p llmff --test cli_run ollama`

Expected: FAIL because the CLI does not accept `--ollama` and backends list does not include it.

- [ ] **Step 3: Write minimal implementation**

Import `OllamaBackend`, add `ollama: Vec<String>` to `Run`, register each alias with `Engine::with_backend`, and print `ollama` from `backends list`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p llmff --test cli_run ollama`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/llmff-cli/src/commands.rs crates/llmff-cli/tests/cli_run.rs
git commit -m "feat: register ollama backends from cli"
```

### Task 3: Document And Verify

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Document Ollama registration**

Add an example:

```bash
llmff run pipeline.yaml --ollama ollama=http://localhost:11434
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
git add README.md docs/superpowers/specs/2026-05-22-ollama-backend-design.md docs/superpowers/plans/2026-05-22-ollama-backend.md
git commit -m "docs: document ollama backend"
```
