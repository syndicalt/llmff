# File-Backed Resources Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add manifest-relative file-backed JSON schemas and system prompts while preserving inline schema compatibility.

**Architecture:** Extend `StageSpec` with `schema_path`. Keep deterministic stage behavior in `llmff-core::stage`, using the engine-provided `cwd` for resource resolution. Keep CLI behavior unchanged except examples and docs.

**Tech Stack:** Rust, existing `tempfile`, `serde`, `jsonschema`, and CLI integration tests.

---

## Task 1: Manifest Supports schema_path

**Files:**
- Modify: `crates/llmff-core/src/manifest.rs`

- [ ] Write a failing manifest parsing test asserting `schema_path` deserializes.
- [ ] Run: `cargo test -p llmff-core manifest::tests::parses_schema_path`
- [ ] Add `schema_path: Option<String>` to `StageSpec`.
- [ ] Run: `cargo test -p llmff-core manifest::tests::parses_schema_path`
- [ ] Commit: `feat: parse schema_path in manifests`

## Task 2: validate_json Loads schema_path

**Files:**
- Modify: `crates/llmff-core/src/stage.rs`

- [ ] Write a failing test using a temp directory schema file and `schema_path`.
- [ ] Run: `cargo test -p llmff-core stage::tests::validate_json_loads_schema_path`
- [ ] Implement schema source resolution: inline `schema` first, then `schema_path`.
- [ ] Run: `cargo test -p llmff-core stage::tests::validate_json_loads_schema_path`
- [ ] Add and verify a missing-file error test.
- [ ] Commit: `feat: load JSON schemas from files`

## Task 3: system Loads path

**Files:**
- Modify: `crates/llmff-core/src/stage.rs`

- [ ] Write a failing test proving `system path` prepends file text to parent text.
- [ ] Run: `cargo test -p llmff-core stage::tests::system_stage_prepends_file_text`
- [ ] Implement `system` path loading relative to `cwd`.
- [ ] Run: `cargo test -p llmff-core stage::tests::system_stage_prepends_file_text`
- [ ] Run: `cargo test -p llmff-core stage::tests`
- [ ] Commit: `feat: load system prompts from files`

## Task 4: Examples And README

**Files:**
- Modify: `examples/json-repair.yaml`
- Add: `examples/policy.md`
- Modify: `README.md`
- Modify: `crates/llmff-cli/tests/cli_run.rs`

- [ ] Write a failing CLI integration assertion that the example manifest still inspects and runs after switching to `schema_path`.
- [ ] Run: `cargo test -p llmff --test cli_run inspect_example_manifest_succeeds`
- [ ] Update example manifest to use `schema_path: ./answer.schema.json` and a `system` stage with `path: ./policy.md`.
- [ ] Update README examples to mention file-backed resources.
- [ ] Run: `cargo test -p llmff --test cli_run`
- [ ] Commit: `docs: use file-backed resources in examples`

## Final Verification

- [ ] Run: `cargo fmt --all --check`
- [ ] Run: `cargo test --workspace`
- [ ] Run: `cargo run -p llmff -- inspect examples/json-repair.yaml`
- [ ] Run the mock example with trace:

```bash
rm -f examples/answer.json /tmp/llmff-trace.jsonl
LLMFF_MOCK_BAD_RESPONSE='{"wrong":true}' \
LLMFF_MOCK_GOOD_RESPONSE='{"answer":"ok"}' \
cargo run -p llmff -- run examples/json-repair.yaml --trace /tmp/llmff-trace.jsonl
```

- [ ] Confirm `examples/answer.json` contains `{"answer":"ok"}`.
- [ ] Remove generated `examples/answer.json`.
