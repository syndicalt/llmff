# Parallel Scheduler Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add opt-in parallel execution for independent ready stages while preserving sequential execution as the default.

**Architecture:** Keep graph validation and dependency discovery in the core. Add a `SchedulerMode` to `RunOptions`; sequential mode keeps the existing behavior, parallel mode repeatedly runs graph-ready batches with a stable completed-status snapshot and deterministic trace emission.

**Tech Stack:** Rust workspace, `tokio` time for async concurrency tests, `futures::future::join_all` for borrowed concurrent stage futures, existing engine and CLI tests.

---

## File Structure

- Modify `Cargo.toml`: add direct `futures` dependency and enable `tokio` `time`/`sync` features used by tests.
- Modify `crates/llmff-core/Cargo.toml`: use workspace `futures`.
- Modify `crates/llmff-core/src/graph.rs`: expose stage dependency discovery inside the crate.
- Modify `crates/llmff-core/src/engine.rs`: add `SchedulerMode`, parallel batch execution, and core tests.
- Modify `crates/llmff-cli/src/commands.rs`: add `llmff run --parallel`.
- Modify `crates/llmff-cli/tests/cli_run.rs`: add CLI coverage for the flag.
- Modify `README.md`: document `--parallel` and update limitations.

## Task 1: Core Parallel Scheduler

- [x] **Step 1: Write failing default sequential scheduler test**

Add `default_scheduler_runs_ready_model_stages_sequentially` in `crates/llmff-core/src/engine.rs`. Use a `DelayedBackend` that increments an active counter, awaits `tokio::time::sleep(Duration::from_millis(25))`, then decrements. A manifest with two independent `infer` stages from the same `load` should leave `max_active == 1` under default options.

- [x] **Step 2: Write failing parallel scheduler test**

Add `parallel_scheduler_runs_ready_model_stages_concurrently`. Use the same manifest and backend, but pass `RunOptions { scheduler: SchedulerMode::Parallel, .. }`. Expect `max_active == 2`.

- [x] **Step 3: Run RED**

Run `cargo test -p llmff-core scheduler_runs_ready_model_stages`.

Expected: FAIL because `SchedulerMode` and parallel execution do not exist.

- [x] **Step 4: Implement core scheduler**

Add `SchedulerMode`, `RunOptions.scheduler`, parallel ready-batch execution using `futures::future::join_all`, and crate-visible graph dependency discovery.

- [x] **Step 5: Run GREEN and commit**

Run `cargo test -p llmff-core scheduler_runs_ready_model_stages`.

Commit `feat: add opt-in parallel scheduler`.

## Task 2: CLI Flag and Documentation

- [x] **Step 1: Write failing CLI test**

Add a CLI integration test showing `llmff run --parallel <manifest>` succeeds with mock backends.

- [x] **Step 2: Run RED**

Run `cargo test -p llmff --test cli_run run_accepts_parallel_scheduler_flag`.

Expected: FAIL because `--parallel` is not accepted yet.

- [x] **Step 3: Implement CLI flag**

Add `parallel: bool` to the `Run` command and set `RunOptions.scheduler` accordingly.

- [x] **Step 4: Run GREEN, document, and verify**

Run focused CLI test, `cargo fmt --all --check`, `cargo test --workspace`, and `cargo run -p llmff -- inspect examples/json-repair.yaml`.

- [x] **Step 5: Commit**

Commit `docs: document parallel scheduler`.

## Self-Review

- Spec coverage: opt-in core scheduler, deterministic trace semantics, CLI flag, docs, and verification are covered.
- Placeholder scan: no placeholder implementation steps remain.
- Type consistency: plan uses `SchedulerMode`, `RunOptions.scheduler`, and `--parallel` consistently.
