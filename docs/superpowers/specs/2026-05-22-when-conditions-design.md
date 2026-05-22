# When Conditions Design

## Purpose

Make the manifest `when` field executable. The manifest already parses `when`, examples use `when: invalid`, and the runtime already has `StageStatus::Skipped`, but the engine currently ignores `when`.

This slice adds conditional stage execution so a stage can run only when its parent status matches a declared status.

## Manifest Shape

```yaml
graph:
  - id: repair
    op: repair
    from: validate
    when: invalid
    model: mock:good
```

Supported values:

- `success`
- `invalid`
- `skipped`

## Runtime Semantics

- A stage without `when` behaves as it does today.
- A stage with `when` checks the status of its `from` source before any stage-specific work.
- If the parent status matches, the stage executes normally.
- If the parent status does not match, the stage returns `StageStatus::Skipped`.
- Skipped stages still emit normal trace lifecycle events with status `skipped`.
- Unsupported `when` values are dry-run validation errors.
- A stage with `when` must have `from`; this is already required for all current non-`load` stage types, and `load` with `when` is rejected.

## Design Choice

Conditional execution belongs in the engine, not individual stage implementations. This keeps semantics uniform for model stages, deterministic stages, tools, writes, and future built-ins.

The condition check happens at the top of `Engine::execute_stage`, before dispatching to operation-specific execution. This guarantees skipped stages do not call model backends, tools, HTTP endpoints, or filesystem side effects.

## Compatibility

The existing `examples/json-repair.yaml` already routes between `validate` and `repair`, so `repair when: invalid` remains correct:

- invalid validation: `repair` runs, route chooses repair.
- successful validation: `repair` skips, route chooses validate.

Top-level outputs that point directly at a skipped stage still fail, preserving the existing rule that outputs require successful values.

## Acceptance Criteria

- `when: invalid` skips a repair stage when its parent status is success.
- Skipped model stages do not call their backend.
- `when: invalid` runs a repair stage when its parent status is invalid.
- `llmff inspect` rejects unsupported `when` values.
- Traces record skipped stages as `status: "skipped"`.
- `cargo fmt --all --check`, `cargo test --workspace`, and `cargo run -p llmff -- inspect examples/json-repair.yaml` pass.
