# Aspirational Loop Operations Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the operations and loop-state features needed to make the aspirational v1.1 loop examples first-class, runnable llmff manifests while preserving bounded, typed, inspectable DAG execution.

**Architecture:** Keep each capability as an explicit stage or explicit loop-stage option in the existing `StageSpec` model. Add deterministic JSON stages first, extend loop output retention and carry semantics second, then add bounded collection mapping as a separate primitive. Do not add a general expression language, top-level workflow language, autonomous planner, memory layer, or unbounded loop mode.

**Tech Stack:** Rust, serde/serde_yaml, serde_json, jsonschema, tokio, existing llmff graph validation, engine dispatch, trace/event context, schema contract checks, CLI integration tests, and example catalog tests.

---

## Scope

Implement these nine features:

1. `op: extract`
2. `op: score`
3. `op: select`
4. Loop iteration retention
5. Loop carry/feedback polish
6. `op: predicate`
7. Stronger `op: tool` loop contracts and examples
8. `op: accumulate`
9. `op: map`

## Non-Goals

- No unbounded loops.
- No nested loops or nested maps in this release slice.
- No arbitrary scripting, expression language, or plugin strategy API.
- No speculative branch execution.
- No default parallel execution. `map.parallel` is opt-in and must honor a concurrency cap.
- No llmff-owned memory or project-management layer. These stages remain execution primitives below supervisors such as Pathlight/EventLoom.

## Release Slices

- Slice A: Shared JSON path utility plus `extract`, `predicate`, and `accumulate`.
- Slice B: Loop retention, carry initialization, `score`, and `select`.
- Slice C: Tool-loop contracts and upgraded aspirational examples.
- Slice D: Bounded `map`, optional parallel map execution, and final docs/release gates.

Each slice must leave these commands passing before the next slice starts:

```bash
cargo fmt --all --check
cargo test --workspace
python3 scripts/check-schema-contract.py
```

Run `cargo clippy --workspace --all-targets -- -D warnings` before merging the full feature branch.

## File Structure

- Modify `crates/llmff-core/src/manifest.rs`: add stage fields and typed loop retention config.
- Modify `docs/schemas/pipeline-manifest-v1.schema.json`: publish the new manifest surface.
- Modify `crates/llmff-core/src/stage.rs`: add built-in metadata and deterministic dispatch.
- Create `crates/llmff-core/src/stage/json_path.rs`: shared minimal JSON path helper.
- Create `crates/llmff-core/src/stage/extract.rs`.
- Create `crates/llmff-core/src/stage/predicate.rs`.
- Create `crates/llmff-core/src/stage/accumulate.rs`.
- Create `crates/llmff-core/src/stage/score.rs`.
- Create `crates/llmff-core/src/stage/select.rs`.
- Modify `crates/llmff-core/src/engine.rs`: dispatch new stages, retain loop iterations, and execute `map`.
- Modify `crates/llmff-core/src/graph.rs`: validate new stage contracts and map/loop body references.
- Modify `crates/llmff-core/src/engine/scheduler.rs`: support opt-in map concurrency if map is implemented outside the main engine loop.
- Modify `crates/llmff-core/src/trace.rs`, `docs/schemas/trace-v1.schema.json`, and `docs/schemas/event-v1.schema.json`: add map context fields if map emits child-stage trace events.
- Modify `crates/llmff-cli/src/commands.rs` and `docs/schemas/inspect-report-v1.schema.json` if inspect metadata changes.
- Modify `crates/llmff-cli/tests/cli_run.rs` and `crates/llmff-cli/tests/example_catalog.rs`.
- Modify `examples/loops/*.yaml` and add one map example under `examples/loops/`.
- Modify `examples/loops/README.md`, `examples/README.md`, `docs/execution.md`, `docs/pipeline-library.md`, `docs/agent-workflows.md`, `SPEC.md`, and the next release notes.

## Manifest Additions

Add optional fields to `StageSpec` only where they are needed by concrete stage contracts:

```rust
pub json_path: Option<String>,
pub mode: Option<String>,
pub criteria: Option<String>,
pub score_field: Option<String>,
pub reason_field: Option<String>,
pub label_field: Option<String>,
pub min_score: Option<f64>,
pub max_score: Option<f64>,
pub value: Option<serde_json::Value>,
pub limit: Option<usize>,
pub dedupe_field: Option<String>,
pub initial_carry: BTreeMap<String, serde_json::Value>,
pub items_from: Option<String>,
pub max_items: Option<usize>,
pub parallel: Option<bool>,
pub max_concurrency: Option<usize>,
```

Replace the existing `retain_iterations: Option<String>` with a backward-compatible untagged enum:

```rust
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum LoopRetentionSpec {
    Mode(String),
    Config {
        mode: String,
        #[serde(default)]
        stages: Vec<String>,
        include_values: Option<bool>,
    },
}
```

Accepted retention modes:

- `none`: current v1.1 output shape.
- `summaries`: per-iteration statuses and selected metadata without values.
- `all`: retained values for all body stages.
- object config: retain only listed body stages, with `include_values` controlling payload retention.

---

### Task 1: Shared JSON Path Utility

**Files:**
- Create `crates/llmff-core/src/stage/json_path.rs`
- Modify `crates/llmff-core/src/stage.rs`

- [ ] Add tests for top-level fields, nested object paths, array indexes, empty-path root selection, missing paths, and invalid empty segments.
- [ ] Implement a deliberately small path grammar: dot-separated object keys and numeric array indexes, for example `result.answer`, `items.0.score`, and `metadata.iterations_run`.
- [ ] Return `Option<&serde_json::Value>` for reads and cloned values only at call sites that need ownership.
- [ ] Wire the helper module into `stage.rs`.
- [ ] Verify:

```bash
cargo test -p llmff-core stage::json_path
```

- [ ] Commit:

```bash
git add crates/llmff-core/src/stage.rs crates/llmff-core/src/stage/json_path.rs
git commit -m "feat: add json path helper"
```

### Task 2: `op: extract`

**Contract:**

- Required: `from`, and one of `field` or `json_path`.
- Input: JSON value, or text/messages that parse as JSON.
- Output: `Value::Json` preserving the extracted JSON type.
- Missing path: stage execution error.

**Files:**
- Modify `crates/llmff-core/src/manifest.rs`
- Modify `docs/schemas/pipeline-manifest-v1.schema.json`
- Modify `crates/llmff-core/src/stage.rs`
- Create `crates/llmff-core/src/stage/extract.rs`
- Modify `crates/llmff-core/src/engine.rs`
- Modify `crates/llmff-core/src/graph.rs`
- Modify `crates/llmff-cli/tests/cli_run.rs`

- [ ] Add manifest parser tests for `json_path`.
- [ ] Add stage unit tests for nested extraction, scalar extraction, array-index extraction, missing input, and missing path.
- [ ] Add graph validation requiring `from` and `field|json_path`.
- [ ] Add schema property `json_path` and op-specific examples.
- [ ] Add deterministic dispatch and built-in metadata.
- [ ] Add CLI test that loads JSON, extracts `result.final_answer`, and writes the extracted JSON value.
- [ ] Verify:

```bash
cargo test -p llmff-core manifest::tests::parses_extract_json_path_field
cargo test -p llmff-core stage::extract
cargo test -p llmff-cli --test cli_run run_extracts_nested_json_field
python3 scripts/check-schema-contract.py
```

- [ ] Commit:

```bash
git add crates/llmff-core/src/manifest.rs docs/schemas/pipeline-manifest-v1.schema.json crates/llmff-core/src/stage.rs crates/llmff-core/src/stage/extract.rs crates/llmff-core/src/engine.rs crates/llmff-core/src/graph.rs crates/llmff-cli/tests/cli_run.rs
git commit -m "feat: add extract stage"
```

### Task 3: `op: predicate`

**Contract:**

- Required: `from`.
- Path source: `field` or `json_path`; if omitted, evaluate the whole input JSON value.
- Supported `mode`: `truthy`, `exists`, `equals`, `gt`, `gte`, `lt`, `lte`, `contains`.
- `equals` and numeric comparison modes require `value`.
- Output:

```json
{
  "passed": true,
  "mode": "gte",
  "path": "score",
  "observed": 8,
  "expected": 7
}
```

**Files:**
- Modify `crates/llmff-core/src/manifest.rs`
- Modify `docs/schemas/pipeline-manifest-v1.schema.json`
- Modify `crates/llmff-core/src/stage.rs`
- Create `crates/llmff-core/src/stage/predicate.rs`
- Modify `crates/llmff-core/src/engine.rs`
- Modify `crates/llmff-core/src/graph.rs`

- [ ] Add parser tests for `mode` and `value`.
- [ ] Add unit tests for each predicate mode and for invalid mode/value combinations.
- [ ] Add validation requiring `value` for `equals`, `gt`, `gte`, `lt`, and `lte`.
- [ ] Add deterministic dispatch and metadata.
- [ ] Add docs showing loop break usage:

```yaml
break_on:
  type: field_true
  stage: ready
  field: passed
```

- [ ] Verify:

```bash
cargo test -p llmff-core stage::predicate
cargo test -p llmff-core graph::tests::validates_predicate_stage_contract
python3 scripts/check-schema-contract.py
```

- [ ] Commit:

```bash
git add crates/llmff-core/src/manifest.rs docs/schemas/pipeline-manifest-v1.schema.json crates/llmff-core/src/stage.rs crates/llmff-core/src/stage/predicate.rs crates/llmff-core/src/engine.rs crates/llmff-core/src/graph.rs
git commit -m "feat: add predicate stage"
```

### Task 4: `op: accumulate`

**Contract:**

- Required: `from`.
- Supported `mode`: `append`, `extend`, `merge_object`.
- Optional: `limit`, `dedupe_field`.
- Input for prior state comes from normal graph/carry wiring, not hidden global state.
- Output: JSON array for `append` and `extend`, JSON object for `merge_object`.

**Files:**
- Modify `crates/llmff-core/src/manifest.rs`
- Modify `docs/schemas/pipeline-manifest-v1.schema.json`
- Modify `crates/llmff-core/src/stage.rs`
- Create `crates/llmff-core/src/stage/accumulate.rs`
- Modify `crates/llmff-core/src/engine.rs`
- Modify `crates/llmff-core/src/graph.rs`

- [ ] Add tests for append, extend, merge-object, limit truncation, and `dedupe_field`.
- [ ] Add `initial_carry` to loop stages so the first iteration can seed accumulator aliases such as `history: []`.
- [ ] Validate `initial_carry` keys do not collide with body stage ids.
- [ ] Validate `dedupe_field` is only accepted for array outputs.
- [ ] Document the pattern:

```yaml
initial_carry:
  history: []
carry:
  history: update_history
```

- [ ] Verify:

```bash
cargo test -p llmff-core stage::accumulate
cargo test -p llmff-core loop_initial_carry_seeds_first_iteration
python3 scripts/check-schema-contract.py
```

- [ ] Commit:

```bash
git add crates/llmff-core/src/manifest.rs docs/schemas/pipeline-manifest-v1.schema.json crates/llmff-core/src/stage.rs crates/llmff-core/src/stage/accumulate.rs crates/llmff-core/src/engine.rs crates/llmff-core/src/graph.rs
git commit -m "feat: add accumulate stage"
```

### Task 5: Loop Iteration Retention and Carry Polish

**Contract:**

- Existing `retain_iterations: "none"|"summaries"|"all"` manifests keep parsing.
- New object form can retain only selected body stages.
- Loop output keeps existing `final` and `metadata`.
- When retention is enabled, loop output also includes `iterations`.
- Carry aliases are available during iteration 1 when seeded by `initial_carry`; otherwise they become available after the first producing iteration.

**Files:**
- Modify `crates/llmff-core/src/manifest.rs`
- Modify `docs/schemas/pipeline-manifest-v1.schema.json`
- Modify `crates/llmff-core/src/graph.rs`
- Modify `crates/llmff-core/src/engine.rs`
- Modify `docs/execution.md`

- [ ] Add parser tests for string and object `retain_iterations`.
- [ ] Add graph validation for retention stage names.
- [ ] Add engine tests for `none`, `summaries`, `all`, selected-stage retention, and `include_values: false`.
- [ ] Add engine tests for `initial_carry` and carry aliases across iterations.
- [ ] Keep retained values serialized through the same `Value` conversion code used by outputs.
- [ ] Ensure traces still record every loop body stage even when output retention is `none`.
- [ ] Verify:

```bash
cargo test -p llmff-core loop_retains_iteration_summaries
cargo test -p llmff-core loop_retains_selected_iteration_stage_outputs
cargo test -p llmff-core loop_initial_carry_seeds_first_iteration
python3 scripts/check-schema-contract.py
```

- [ ] Commit:

```bash
git add crates/llmff-core/src/manifest.rs docs/schemas/pipeline-manifest-v1.schema.json crates/llmff-core/src/graph.rs crates/llmff-core/src/engine.rs docs/execution.md
git commit -m "feat: retain loop iterations"
```

### Task 6: `op: score`

**Contract:**

- Required: `from`.
- Score source: `score_field`, `field`, or `json_path`.
- Optional: `reason_field`, `label_field`, `min_score`, `max_score`.
- Output normalizes score-shaped JSON:

```json
{
  "score": 8.0,
  "reason": "passes schema and cites evidence",
  "label": "usable",
  "source": { "...": "original input" }
}
```

- Invalid when score is absent, non-numeric, NaN, or outside min/max bounds.
- This stage does not call an LLM. Use `infer` plus `validate_json` upstream for model judging.

**Files:**
- Modify `crates/llmff-core/src/manifest.rs`
- Modify `docs/schemas/pipeline-manifest-v1.schema.json`
- Modify `crates/llmff-core/src/stage.rs`
- Create `crates/llmff-core/src/stage/score.rs`
- Modify `crates/llmff-core/src/engine.rs`
- Modify `crates/llmff-core/src/graph.rs`

- [ ] Add unit tests for score extraction, reason/label extraction, bounds validation, and invalid score types.
- [ ] Add graph validation requiring a score source.
- [ ] Add docs showing score as a normalizer after a judging `infer` stage.
- [ ] Verify:

```bash
cargo test -p llmff-core stage::score
cargo test -p llmff-core graph::tests::validates_score_stage_contract
python3 scripts/check-schema-contract.py
```

- [ ] Commit:

```bash
git add crates/llmff-core/src/manifest.rs docs/schemas/pipeline-manifest-v1.schema.json crates/llmff-core/src/stage.rs crates/llmff-core/src/stage/score.rs crates/llmff-core/src/engine.rs crates/llmff-core/src/graph.rs
git commit -m "feat: add score stage"
```

### Task 7: `op: select`

**Contract:**

- Required: `from`.
- Input: JSON array, retained loop `iterations`, or object path selected by `json_path`.
- Supported `mode`: `first_success`, `last_success`, `highest_score`, `field_max`, `field_min`.
- Required field options:
  - `highest_score`: `score_field` defaults to `score`.
  - `field_max` and `field_min`: require `field` or `json_path`.
- Output:

```json
{
  "selected": { "...": "chosen item" },
  "metadata": {
    "selected_index": 2,
    "mode": "highest_score",
    "score": 8.0
  }
}
```

**Files:**
- Modify `crates/llmff-core/src/manifest.rs`
- Modify `docs/schemas/pipeline-manifest-v1.schema.json`
- Modify `crates/llmff-core/src/stage.rs`
- Create `crates/llmff-core/src/stage/select.rs`
- Modify `crates/llmff-core/src/engine.rs`
- Modify `crates/llmff-core/src/graph.rs`

- [ ] Add tests for array selection modes.
- [ ] Add tests for selecting from retained loop iterations.
- [ ] Add invalid-input tests for empty arrays, missing score field, and unsupported mode.
- [ ] Add Best-of-N example using `retain_iterations`, `score`, and `select`.
- [ ] Verify:

```bash
cargo test -p llmff-core stage::select
cargo test -p llmff-cli --test example_catalog best_of_n_sampling_selection_loop_inspects
python3 scripts/check-schema-contract.py
```

- [ ] Commit:

```bash
git add crates/llmff-core/src/manifest.rs docs/schemas/pipeline-manifest-v1.schema.json crates/llmff-core/src/stage.rs crates/llmff-core/src/stage/select.rs crates/llmff-core/src/engine.rs crates/llmff-core/src/graph.rs examples/loops
git commit -m "feat: add select stage"
```

### Task 8: Tool Loop Contracts and Examples

**Contract:**

- Keep `op: tool` as the subprocess/tool boundary.
- Do not add agent-planner semantics to `tool`.
- Add documented request and response shape conventions for loop usage.
- Prefer explicit `validate_json` stages around tool inputs and outputs.

**Files:**
- Modify `docs/pipeline-library.md`
- Modify `docs/agent-workflows.md`
- Modify `examples/loops/react-style-tool-use-loop.yaml`
- Modify `examples/loops/README.md`
- Modify `crates/llmff-cli/tests/example_catalog.rs`

- [ ] Define a recommended tool-request schema:

```json
{
  "type": "object",
  "required": ["tool", "args"],
  "properties": {
    "tool": { "type": "string" },
    "args": { "type": "object" },
    "done": { "type": "boolean" }
  }
}
```

- [ ] Define a recommended tool-result schema:

```json
{
  "type": "object",
  "required": ["ok"],
  "properties": {
    "ok": { "type": "boolean" },
    "result": {},
    "error": { "type": "string" }
  }
}
```

- [ ] Update the ReAct-style example to use `predicate`, `tool`, `validate_json`, and `accumulate`.
- [ ] Add a small checked-in fixture tool command that is deterministic and offline-safe for tests.
- [ ] Add catalog tests that inspect the tool-loop example and, when feasible, run it with the mock backend.
- [ ] Verify:

```bash
cargo test -p llmff-cli --test example_catalog react_style_tool_use_loop_inspects
cargo test --workspace
```

- [ ] Commit:

```bash
git add docs/pipeline-library.md docs/agent-workflows.md examples/loops/react-style-tool-use-loop.yaml examples/loops/README.md crates/llmff-cli/tests/example_catalog.rs
git commit -m "docs: define tool loop contracts"
```

### Task 9: `op: map`

**Contract:**

- Required: `from`, `items_from`, `max_items`, and `body`.
- `items_from` points to a JSON array using the same small JSON path grammar.
- Default execution is sequential.
- `parallel: true` enables bounded concurrent item execution with required `max_concurrency`.
- `max_items` must be `>= 1` and caps processed items even if the source array is longer.
- Body stages are namespaced per item in traces.
- Reject nested `loop` and nested `map` in map bodies for this release.
- Output:

```json
{
  "items": [
    {
      "index": 0,
      "status": "success",
      "value": { "...": "body final output" }
    }
  ],
  "metadata": {
    "items_run": 1,
    "items_total": 3,
    "stop_reason": "max_items",
    "parallel": false
  }
}
```

**Files:**
- Modify `crates/llmff-core/src/manifest.rs`
- Modify `docs/schemas/pipeline-manifest-v1.schema.json`
- Modify `crates/llmff-core/src/stage.rs`
- Modify `crates/llmff-core/src/graph.rs`
- Modify `crates/llmff-core/src/engine.rs`
- Modify `crates/llmff-core/src/engine/scheduler.rs` if concurrency is implemented through shared scheduler helpers
- Modify `crates/llmff-core/src/trace.rs`
- Modify `docs/schemas/trace-v1.schema.json`
- Modify `docs/schemas/event-v1.schema.json`
- Add `examples/loops/map-batch-items.yaml`

- [ ] Add parser tests for map fields and map body.
- [ ] Add graph tests for required fields, item path validation, `max_items`, nested loop rejection, nested map rejection, and `parallel/max_concurrency` validation.
- [ ] Add engine tests for sequential map, max item cap, item-stage failures, and output shape.
- [ ] Add engine tests for parallel map preserving deterministic output order by item index.
- [ ] Add trace tests showing `map_id`, `map_index`, and map-body stage ids.
- [ ] Add an offline map example and catalog test.
- [ ] Verify:

```bash
cargo test -p llmff-core map_
cargo test -p llmff-cli --test example_catalog map_batch_items_inspects
python3 scripts/check-schema-contract.py
```

- [ ] Commit:

```bash
git add crates/llmff-core/src/manifest.rs docs/schemas/pipeline-manifest-v1.schema.json crates/llmff-core/src/stage.rs crates/llmff-core/src/graph.rs crates/llmff-core/src/engine.rs crates/llmff-core/src/engine/scheduler.rs crates/llmff-core/src/trace.rs docs/schemas/trace-v1.schema.json docs/schemas/event-v1.schema.json examples/loops/map-batch-items.yaml crates/llmff-cli/tests/example_catalog.rs
git commit -m "feat: add bounded map stage"
```

### Task 10: Upgrade Aspirational Examples and Documentation

**Files:**
- Modify `examples/loops/self-refining-answer-loop.yaml`
- Modify `examples/loops/react-style-tool-use-loop.yaml`
- Modify `examples/loops/best-of-n-sampling+selection-loop.yaml`
- Modify `examples/loops/iterative-research-fact-check-loop.yaml`
- Modify `examples/loops/README.md`
- Modify `examples/README.md`
- Modify `docs/execution.md`
- Modify `docs/pipeline-library.md`
- Modify `docs/agent-workflows.md`
- Modify `SPEC.md`
- Modify next release notes

- [ ] Self-refine example uses `predicate` and `extract`.
- [ ] ReAct-style tool loop uses `predicate`, `tool`, `validate_json`, and `accumulate`.
- [ ] Best-of-N example uses `retain_iterations`, `score`, and `select`.
- [ ] Iterative research example uses `retrieve`, `accumulate`, `predicate`, and `extract`.
- [ ] Map example uses `op: map` for bounded batch item execution.
- [ ] Docs state the execution boundary: llmff repeats bounded graph fragments; supervisors decide product-level policy and long-running orchestration.
- [ ] Docs state that all loop/map examples are bounded by `max_iterations` or `max_items`.
- [ ] Verify:

```bash
cargo run -p llmff -- inspect examples/loops/self-refining-answer-loop.yaml
cargo run -p llmff -- inspect examples/loops/react-style-tool-use-loop.yaml
cargo run -p llmff -- inspect "examples/loops/best-of-n-sampling+selection-loop.yaml"
cargo run -p llmff -- inspect examples/loops/iterative-research-fact-check-loop.yaml
cargo run -p llmff -- inspect examples/loops/map-batch-items.yaml
cargo test -p llmff-cli --test example_catalog
```

- [ ] Commit:

```bash
git add examples/loops examples/README.md docs/execution.md docs/pipeline-library.md docs/agent-workflows.md SPEC.md docs/release-notes
git commit -m "docs: upgrade aspirational loop examples"
```

### Task 11: Full Verification and Release Prep

- [ ] Run formatting:

```bash
cargo fmt --all --check
```

- [ ] Run linting:

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

- [ ] Run tests:

```bash
cargo test --workspace
```

- [ ] Run schema contract check:

```bash
python3 scripts/check-schema-contract.py
```

- [ ] Inspect all loop examples:

```bash
for file in examples/loops/*.yaml; do
  cargo run -p llmff -- inspect "$file"
done
```

- [ ] Run release preflight using the target release version:

```bash
scripts/release-preflight.sh <version>
```

- [ ] Confirm no stale planning markers remain in new docs:

```bash
rg -n "T[B]D|T[O]DO|FIX[M]E|implement[ ]later|stub[ ]text" docs examples crates
```

- [ ] Commit any final fixes:

```bash
git add crates docs examples scripts
git commit -m "chore: prepare loop operations release"
```

## Self-Review Checklist

- [ ] Each new operation has manifest parsing, JSON schema coverage, graph validation, execution tests, and documentation.
- [ ] The loop examples are runnable or clearly marked inspect-only when they require a live model or external tool.
- [ ] Loop and map execution remain bounded by required integer caps.
- [ ] Retained iteration output is opt-in and does not change the default loop output shape.
- [ ] Trace/event fields are additive.
- [ ] Parallel map output order is deterministic by item index.
- [ ] The docs preserve llmff's boundary as a bounded execution engine, not an all-in-one agent platform.
- [ ] All verification commands in Task 11 pass before merge, PR, or release tagging.
