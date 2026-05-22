# Backend CLI Configuration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add explicit FFmpeg-like CLI backend registration for OpenAI-compatible servers without making environment variables mandatory.

**Architecture:** Keep backend resolution in `llmff-core::Engine`, and keep CLI parsing/config assembly in `llmff-cli::commands`. Exact backend keys preserve mock behavior; alias keys support real backend model ids such as `openai:gpt-4.1-mini`.

**Tech Stack:** Rust, Clap, existing `OpenAiCompatibleBackend`, `wiremock`, `assert_cmd`.

---

## Task 1: Alias Backend Resolution

**Files:**
- Modify: `crates/llmff-core/src/engine.rs`
- Test: `crates/llmff-core/src/engine.rs`

- [ ] Write a failing unit test proving a backend registered as `openai` receives provider model `gpt-test` when the manifest model is `openai:gpt-test`.
- [ ] Run: `cargo test -p llmff-core engine::tests::alias_backend_receives_provider_model_id`
- [ ] Implement exact-match-first, alias-match-second backend resolution in `Engine`.
- [ ] Run: `cargo test -p llmff-core engine::tests::alias_backend_receives_provider_model_id`
- [ ] Commit: `feat: resolve model backend aliases`

## Task 2: CLI Backend Flag Parsing

**Files:**
- Modify: `crates/llmff-cli/src/commands.rs`
- Test: `crates/llmff-cli/src/commands.rs`

- [ ] Write failing tests for parsing `alias=value` pairs and rejecting malformed values.
- [ ] Run: `cargo test -p llmff cli_backend_config`
- [ ] Implement small parsing helpers for `--backend`, `--api-key-env`, and `--api-key`.
- [ ] Run: `cargo test -p llmff cli_backend_config`
- [ ] Commit: `feat: parse backend CLI flags`

## Task 3: CLI OpenAI-Compatible Execution

**Files:**
- Modify: `crates/llmff-cli/src/commands.rs`
- Modify: `crates/llmff-cli/Cargo.toml`
- Test: `crates/llmff-cli/tests/cli_run.rs`

- [ ] Write a failing `wiremock` CLI integration test that runs a manifest with `model: openai:gpt-test` and `--backend openai=<mock server>`.
- [ ] Run: `cargo test -p llmff --test cli_run run_uses_cli_registered_openai_backend`
- [ ] Register `OpenAiCompatibleBackend` from CLI flags.
- [ ] Run: `cargo test -p llmff --test cli_run run_uses_cli_registered_openai_backend`
- [ ] Commit: `feat: run with CLI registered backends`

## Task 4: API Key Env Secret Handling

**Files:**
- Modify: `crates/llmff-cli/src/commands.rs`
- Test: `crates/llmff-cli/tests/cli_run.rs`

- [ ] Write a failing CLI integration test proving `--api-key-env openai=LLMFF_TEST_API_KEY` sends `Authorization: Bearer <value>`.
- [ ] Assert stdout and stderr do not contain the secret value.
- [ ] Run: `cargo test -p llmff --test cli_run run_uses_api_key_env_without_printing_secret`
- [ ] Implement API key resolution with literal key taking precedence over env-key lookup.
- [ ] Run: `cargo test -p llmff --test cli_run run_uses_api_key_env_without_printing_secret`
- [ ] Commit: `feat: support backend API key indirection`

## Task 5: Docs And Verification

**Files:**
- Modify: `README.md`
- Test: workspace

- [ ] Update README backend notes to show CLI-first backend registration and secret indirection.
- [ ] Run: `cargo fmt --all --check`
- [ ] Run: `cargo test --workspace`
- [ ] Run: `cargo run -p llmff -- backends list`
- [ ] Run: `cargo run -p llmff -- inspect examples/json-repair.yaml`
- [ ] Commit: `docs: document CLI backend registration`
