# Chat Messages Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preserve chat messages and roles through `system` and `infer` so chat backends receive structured system/user messages.

**Architecture:** Add `Message` and `Value::Messages` in the core value model. Change backend requests from a flattened prompt string to structured messages, with engine helpers converting existing text/JSON parents into a user message. Keep existing deterministic stages compatible by rendering messages to text at stage boundaries that still require text.

**Tech Stack:** Rust workspace, `serde`, existing backend adapters, `wiremock`, engine and stage unit tests, CLI integration tests.

---

## File Structure

- Modify `crates/llmff-core/src/value.rs`: add `Message` and `Value::Messages`.
- Modify `crates/llmff-core/src/stage.rs`: make `system` produce messages when a policy file is present and render messages for deterministic consumers.
- Modify `crates/llmff-core/src/backend.rs`: replace prompt-only requests with message requests and serialize provider messages.
- Modify `crates/llmff-core/src/engine.rs`: pass messages into infer/repair and keep type validation aware of messages.
- Modify `README.md`: document that system stages preserve chat roles for chat backends.

## Task 1: Message Value and System Stage

- [x] **Step 1: Write failing `system_stage_creates_chat_messages_from_policy_file` test**

Add a stage unit test that creates `policy.md`, runs `system` with text input, and expects `StageStatus::Success(Value::Messages(vec![Message { role: "system", content: "Use terse JSON." }, Message { role: "user", content: "Return an answer." }]))`.

- [x] **Step 2: Run RED**

Run `cargo test -p llmff-core stage::tests::system_stage_creates_chat_messages_from_policy_file`.

- [x] **Step 3: Implement message value and system behavior**

Add `Message` and `Value::Messages`. Update `system` to create messages when `path` is present.

- [x] **Step 4: Run GREEN and commit**

Run `cargo test -p llmff-core stage::tests::system_stage_creates_chat_messages_from_policy_file`.

Commit `feat: add chat message values`.

## Task 2: Backend Message Requests

- [x] **Step 1: Write failing backend HTTP assertions**

Update OpenAI-compatible and Ollama backend tests to assert the request body contains separate `system` and `user` messages.

- [x] **Step 2: Run RED**

Run the two backend tests.

- [x] **Step 3: Implement `InferRequest.messages`**

Replace prompt-only backend request construction with structured messages. Update mock, OpenAI-compatible, and Ollama backends.

- [x] **Step 4: Run GREEN and commit**

Run the focused backend tests and commit `feat: send chat messages to backends`.

## Task 3: Engine Propagation and Compatibility

- [ ] **Step 1: Write failing engine test**

Add a backend test double proving `infer` receives two messages after a `system` stage.

- [ ] **Step 2: Run RED**

Run `cargo test -p llmff-core engine::tests::infer_receives_system_and_user_messages`.

- [ ] **Step 3: Implement engine message conversion**

Add helpers that convert `Text`, `Json`, or `Messages` parent values into backend messages. Update `infer`, `repair`, template/tool/write/validation text rendering only where needed.

- [ ] **Step 4: Run GREEN, document, and verify**

Run focused engine test, `cargo fmt --all --check`, `cargo test --workspace`, and `cargo run -p llmff -- inspect examples/json-repair.yaml`.

- [ ] **Step 5: Commit**

Commit `docs: document chat message preservation`.

## Self-Review

- Spec coverage: message value, system behavior, backend serialization, engine propagation, docs, and verification are covered.
- Placeholder scan: no placeholder implementation steps remain.
- Type consistency: plan uses `Message`, `Value::Messages`, and `InferRequest.messages` consistently.
