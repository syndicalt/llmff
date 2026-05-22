# Dependency-Order Execution Design

## Purpose

Move `llmff` from manifest-order execution to graph dependency-order execution. This makes the durable primitive the inference graph rather than the order in which stages happen to be written.

The current engine validates and executes stages in manifest order. That blocks natural fan-out/fan-in manifests and forces route targets to appear before the route stage. This slice keeps execution sequential, but it computes a deterministic topological order from declared dependencies.

## User Shape

Manifest authors may write stages in any order as long as references form an acyclic graph:

```yaml
version: 1
inputs:
  prompt:
    path: ./question.txt
graph:
  - id: save
    op: write
    from: draft
    path: ./answer.txt
  - id: draft
    op: infer
    from: load_prompt
    model: mock:good
  - id: load_prompt
    op: load
    input: prompt
```

`llmff run` executes `load_prompt`, then `draft`, then `save`. `llmff inspect` accepts the manifest and rejects cycles or missing references.

## Dependency Model

Every stage may depend on other stages:

- `from` is a dependency for every stage that declares it.
- Route target references are also dependencies because this slice preserves the current route semantics: a route chooses between already-computed stage statuses.

Top-level `outputs` do not affect execution ordering. They are resolved after all stages have executed, as they are today.

## Ordering Rules

- Preserve deterministic ordering: when multiple stages are ready at the same time, use their original manifest order.
- Reject duplicate stage ids.
- Reject unknown stage references, route targets, and output references after collecting all stage ids, so forward references are legal but missing references are not.
- Reject cycles with an error that includes `cycle detected in graph`.
- Keep execution sequential in this slice. Parallel scheduling can build on the same normalized dependency order later.

## Architecture

Extend `Graph` so it owns stages in validated dependency order:

- `Graph::from_manifest` collects stage ids first.
- It validates all references against the full id set.
- It computes dependency-order stages with Kahn's algorithm.
- It stores the sorted stages in `Graph::stages`.

No CLI pipeline semantics move into the CLI. `Engine::validate_manifest`, `run_manifest_with_options`, and `inspect` continue to consume the normalized `Graph`.

## Compatibility

Existing manifest-order pipelines still run. The route-stage design note's earlier-order restriction is superseded by this scheduler upgrade; route targets may now appear anywhere in the manifest as long as the graph is acyclic.

Inline graphs remain linear and continue to parse into already-ordered manifests.

## Acceptance Criteria

- Core graph tests prove forward `from` references validate and stages are returned in dependency order.
- Core graph tests prove forward route target references validate and are ordered before the route stage.
- Core graph tests prove cycles are rejected.
- Engine tests prove a manifest written out of dependency order runs successfully.
- README documents that manifest stage order no longer controls execution order.
- `cargo fmt --all --check`, `cargo test --workspace`, and `cargo run -p llmff -- inspect examples/json-repair.yaml` pass.
