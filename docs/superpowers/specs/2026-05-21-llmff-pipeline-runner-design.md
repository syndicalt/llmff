# llmff Pipeline Runner Design

## Purpose

`llmff` is a command-line-first and library-backed inference pipeline runner: an FFmpeg-shaped tool for composing LLM workflows across interchangeable model backends.

The first product slice should prove the core thesis: LLM inference workflows need a portable, reproducible, inspectable pipeline graph that can be run from a shell, embedded from code, and adapted across local and remote runtimes without becoming a high-level agent framework.

## Product Boundary

The first version is a pipeline runner, not a new inference kernel, serving platform, or model conversion suite.

In scope:

- CLI execution of linear and branching inference pipelines.
- A manifest format for reproducible pipeline recipes.
- A graph of typed pipeline stages such as prompt transforms, model inference, validation, routing, repair, retrieval hooks, tool calls, and output writers.
- Backend adapters for existing inference targets.
- Run traces that capture inputs, outputs, timings, backend choices, validation failures, and artifacts.
- A library API that uses the same execution core as the CLI.

Out of scope for the first version:

- Custom CUDA, ROCm, Metal, or WebGPU kernels.
- Native model quantization or format conversion.
- Production multi-tenant serving.
- A full agent framework with memory, planning, or autonomous task management.
- A visual workflow builder.

## Core Thesis

The durable primitive is the inference graph.

Backends will change, model formats will change, and sampling tricks will change. The valuable interface is a stable way to describe, execute, trace, replay, and share workflows that mix model calls with deterministic transformations and external tools.

The tool should feel low-level and composable:

```bash
llmff -i prompt.txt \
  -graph 'load | system(policy.md) | infer(model=fast) | validate(schema=answer.schema.json) | repair(model=strong) | write(answer.json)'
```

Complex graphs can move into version-controlled manifests:

```yaml
version: 1
inputs:
  prompt:
    path: ./question.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: apply_policy
    op: system
    from: load_prompt
    path: ./policy.md
  - id: draft
    op: infer
    from: apply_policy
    model: local:llama-3.1-8b
    temperature: 0.2
  - id: validate
    op: validate_json
    from: draft
    schema: ./answer.schema.json
  - id: repair
    op: repair
    from: validate
    when: invalid
    model: openai:gpt-4.1-mini
outputs:
  final:
    from: repair
    path: ./answer.json
```

## User Experience

The primary interface is a single binary named `llmff`.

Initial command shapes:

```bash
llmff run pipeline.yaml
llmff run -i prompt.txt -g 'load | infer(model=ollama:llama3.1) | write(-)'
llmff inspect pipeline.yaml
llmff trace runs/latest
llmff backends list
llmff stages list
```

The CLI should support stdin and stdout well enough for shell composition:

```bash
cat question.txt | llmff run -g 'load | infer(model=openai:gpt-4.1-mini) | write(-)'
```

The CLI should prefer explicit failure over hidden magic. If a backend, model, schema, input, or tool is missing, the command should fail with a precise diagnostic and a trace entry.

## Architecture

The implementation has four major layers.

### CLI Layer

The CLI parses flags, inline graph strings, and manifest paths. It resolves environment variables and working-directory-relative paths, then hands an execution request to the core engine.

The CLI is not allowed to contain pipeline semantics. It is a thin interface over the library.

### Core Engine

The engine owns graph parsing, validation, scheduling, execution, artifact management, and trace emission.

Core responsibilities:

- Convert manifests and inline expressions into a normalized graph.
- Validate stage ids, input references, type compatibility, required stage parameters, and backend availability.
- Execute stages in dependency order.
- Preserve structured values between stages.
- Emit a deterministic trace for each run.
- Support dry-run validation without invoking models or tools.

The MVP scheduler can execute stages sequentially. The graph model should still support fan-out and fan-in so parallel execution can be added later without changing the manifest format.

### Stage System

Stages are typed operations. Each stage declares:

- Name and version.
- Accepted input value types.
- Output value type.
- Parameter schema.
- Whether it is deterministic.
- Whether it can call a model, tool, network, or filesystem.

Initial built-in stages:

- `load`: read text or JSON from path/stdin.
- `system`: attach or prepend a system instruction.
- `template`: apply variables to a prompt template.
- `infer`: call a configured model backend.
- `validate_json`: validate model output against a JSON Schema.
- `repair`: ask a model to repair invalid structured output.
- `route`: choose a branch based on status or a scalar field.
- `tool`: call an external command or HTTP endpoint through an explicit declaration.
- `write`: write text or JSON to path/stdout.

Retrieval, embedding, reranking, multimodal inputs, scoring, and cache stages are important, but they can follow after the core execution contract is proven.

### Backend Adapters

Backends are model execution providers behind a common interface.

Initial adapters should target:

- OpenAI-compatible HTTP APIs.
- Ollama.
- llama.cpp server.
- vLLM OpenAI-compatible server.

The backend contract should cover:

- Model identifier.
- Chat and completion style requests.
- Streaming token output.
- Sampling parameters.
- Structured response hints where supported.
- Usage and timing metadata where available.

Backend adapters should be shallow at first. `llmff` should orchestrate existing systems before attempting native model loading.

## Data Model

The core engine passes typed values between stages. The MVP value set should be small:

- `Text`: plain text plus optional role metadata.
- `Messages`: chat messages with roles and content parts.
- `Json`: structured JSON value.
- `BinaryRef`: reference to an artifact file.
- `StageStatus`: success, invalid, skipped, or failed with details.

Traces should be written as structured JSON lines so they are easy to inspect, stream, and post-process.

Trace events should include:

- Run id.
- Stage id and operation.
- Start and end timestamps.
- Input references, not necessarily full sensitive input values.
- Output references.
- Backend and model used.
- Token counts and latency when available.
- Validation errors.
- Tool command or endpoint metadata.
- Final status.

## Error Handling

Errors should be explicit, typed, and traceable.

Validation-time errors:

- Invalid manifest syntax.
- Unknown stage operation.
- Missing stage input.
- Type mismatch between stages.
- Missing backend configuration.
- Missing required parameter.

Runtime errors:

- Backend unavailable.
- Model call failed.
- Tool call failed.
- JSON parsing failed.
- Schema validation failed.
- Output write failed.

Stages can either fail the run or produce an invalid status that later stages may handle. For example, `validate_json` can mark output invalid, and `repair` can consume that invalid value. This distinction is central to making pipelines composable.

## Configuration

Configuration should layer in this order:

1. CLI flags.
2. Pipeline manifest values.
3. Environment variables.
4. User config file.
5. Built-in defaults.

Secrets should not be stored in pipeline manifests or traces by default. Backend credentials should come from environment variables or local config.

## Testing Strategy

The first implementation should be test-first around the stable contracts:

- Manifest parsing and normalization.
- Inline graph parsing for the MVP syntax.
- Graph validation errors.
- Stage type compatibility.
- Deterministic built-in stages.
- Mock backend inference.
- JSON validation and repair flow.
- Trace emission shape.
- CLI smoke tests using a mock backend.

Networked backend integration tests should be opt-in so the default test suite remains fast and deterministic.

## MVP Success Criteria

The MVP is successful when a user can:

- Install or run a single CLI.
- Define a pipeline in YAML.
- Run the same pipeline against at least two interchangeable model backends.
- Validate structured JSON output.
- Repair invalid JSON through a stronger fallback model.
- Inspect a trace that explains what happened.
- Pipe stdin to stdout for simple shell workflows.
- Embed the same pipeline runner through a library API.

## Recommended Implementation Shape

Use Rust for the first implementation if starting from a blank repo.

Rationale:

- Single-binary distribution fits the FFmpeg-style tool shape.
- Strong typed boundaries help keep stage contracts and graph validation rigorous.
- Async HTTP support is mature enough for backend adapters.
- The project can later expose C ABI or Python bindings without replacing the core.

Suggested crates:

- `clap` for CLI parsing.
- `serde`, `serde_json`, and `serde_yaml` for manifests and traces.
- `jsonschema` for JSON Schema validation.
- `tokio` and `reqwest` for async backend adapters.
- `thiserror` and `anyhow` split between library errors and CLI reporting.
- `tracing` for internal diagnostics.

## Initial Milestone

Milestone 1 should implement the smallest end-to-end useful loop:

```bash
llmff run examples/json-repair.yaml
```

The example should:

1. Load a prompt.
2. Apply a system instruction.
3. Call a mock or OpenAI-compatible backend.
4. Validate JSON against a schema.
5. Repair invalid JSON through a second backend call.
6. Write the final JSON.
7. Emit a trace.

This milestone proves the product shape without requiring native inference, custom compression, retrieval, or advanced scheduling.
