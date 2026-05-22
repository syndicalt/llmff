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

Manifest stages can reference file-backed resources relative to the manifest:

```yaml
graph:
  - id: apply_policy
    op: system
    from: load_prompt
    path: ./policy.md

  - id: validate
    op: validate_json
    from: draft
    schema_path: ./answer.schema.json
```

## Backend Notes

The CLI keeps backend registration explicit. This keeps commands portable and FFmpeg-like: the command line describes the run, while environment variables are only used when you choose to read a secret by name.

Register an OpenAI-compatible backend:

```bash
llmff run pipeline.yaml \
  --backend openai=https://api.openai.com/v1 \
  --api-key-env openai=OPENAI_API_KEY
```

Then reference that backend alias from a manifest:

```yaml
graph:
  - id: draft
    op: infer
    from: load_prompt
    model: openai:gpt-4.1-mini
```

The model id before the first colon is the backend alias. The model id after the first colon is sent to the provider.

For local OpenAI-compatible servers that do not require auth, omit the key flag:

```bash
llmff run pipeline.yaml \
  --backend local=http://localhost:8000/v1
```

Mock backends remain available for deterministic local runs and tests:

- `LLMFF_MOCK_BAD_RESPONSE`
- `LLMFF_MOCK_GOOD_RESPONSE`

Those mock env vars are convenience fixtures, not the primary backend configuration model.

## Limitations

- Pipeline execution is sequential.
- Schema values are inline JSON strings in the current manifest format.
- Retrieval, embedding, reranking, multimodal values, cache stages, and plugin loading are not implemented yet.
- Native model loading, quantization, and hardware scheduling are out of scope for this MVP.
