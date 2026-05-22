# llmff

`llmff` is an FFmpeg-shaped command-line and library tool for LLM inference pipelines. The MVP focuses on a typed pipeline graph, reproducible YAML manifests, backend adapters, JSON validation and repair, and JSONL traces.

## Current Scope

- `llmff run <manifest>` executes a pipeline manifest.
- `llmff inspect <manifest>` parses and validates graph references.
- `llmff stages list` prints built-in stage names.
- `llmff backends list` prints currently wired backend families.
- The core crate owns execution semantics; the CLI is a thin adapter.
- Mock backends are available for deterministic local runs and tests.
- An OpenAI-compatible backend exists in the core crate for `/v1/chat/completions` servers.

This is not a native inference kernel, model conversion tool, serving platform, or agent framework.

## Example

Run the JSON repair example with deterministic mock model responses:

```bash
LLMFF_MOCK_BAD_RESPONSE='{"wrong":true}' \
LLMFF_MOCK_GOOD_RESPONSE='{"answer":"ok"}' \
cargo run -p llmff -- run examples/json-repair.yaml --trace /tmp/llmff-trace.jsonl
```

Inspect the manifest without running model calls:

```bash
cargo run -p llmff -- inspect examples/json-repair.yaml
```

List built-in stages:

```bash
cargo run -p llmff -- stages list
```

Use stdin/stdout by setting manifest input or output paths to `-`.

## Backend Notes

The CLI currently wires mock backends through:

- `LLMFF_MOCK_BAD_RESPONSE`
- `LLMFF_MOCK_GOOD_RESPONSE`

The core crate includes an `OpenAiCompatibleBackend` for servers that implement `POST /v1/chat/completions`. CLI configuration for real OpenAI-compatible backends is intentionally not exposed until the config model is explicit about secrets and trace redaction.

## Limitations

- Pipeline execution is sequential.
- Schema values are inline JSON strings in the current manifest format.
- Retrieval, embedding, reranking, multimodal values, cache stages, and plugin loading are not implemented yet.
- Native model loading, quantization, and hardware scheduling are out of scope for this MVP.
