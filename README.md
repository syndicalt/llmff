# llmff

`llmff` is an FFmpeg-shaped command-line and library tool for LLM inference pipelines. The MVP focuses on a typed pipeline graph, reproducible YAML manifests, backend adapters, local retrieval, JSON validation and repair, and JSONL traces.

## Install

Install from GitHub:

```bash
cargo install --git https://github.com/syndicalt/llmff llmff
```

Install a tagged release:

```bash
cargo install --git https://github.com/syndicalt/llmff --tag v0.1.1 llmff
```

For a local checkout:

```bash
cargo install --path crates/llmff-cli
```

Verify the installed binary:

```bash
llmff --version
llmff stages list
```

Run the install smoke gate from a checkout:

```bash
scripts/smoke-install.sh --path .
```

Packaged installers for Windows, macOS, Ubuntu, Debian, and Arch Linux are on the roadmap; current releases install through Cargo from GitHub.

## Current Scope

- `llmff run <manifest>` executes a pipeline manifest.
- `llmff inspect <manifest>` dry-run validates graph references, stage requirements, conservative type compatibility, and backend availability.
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
llmff run examples/json-repair.yaml --trace /tmp/llmff-trace.jsonl
```

Run independent ready stages concurrently:

```bash
LLMFF_MOCK_GOOD_RESPONSE='{"answer":"ok"}' \
llmff run --parallel pipeline.yaml
```

Run a compact inline graph:

```bash
LLMFF_MOCK_GOOD_RESPONSE='{"answer":"ok"}' \
llmff run -i examples/question.txt \
  -g 'load | infer(model=mock:good) | write(-)'
```

Run inline retrieval over local documents:

```bash
llmff run -i question.txt \
  -g 'load | retrieve(documents=docs/python.txt;docs/rust.txt,top_k=1) | write(matches.json)'
```

Cache an inline pipeline value across runs:

```bash
llmff run -i question.txt \
  -g 'load | cache(path=.llmff/cache,key=prompt-v1) | write(cached-question.txt)'
```

Inline `load` reads stdin when `-i/--input` is omitted:

```bash
cat examples/question.txt | LLMFF_MOCK_GOOD_RESPONSE='{"answer":"ok"}' \
  llmff run -g 'load | infer(model=mock:good) | write(-)'
```

Inspect the manifest without running model calls:

```bash
llmff inspect examples/json-repair.yaml
```

Inspect an inline graph without running model calls:

```bash
llmff inspect -g 'load | infer(model=mock:good) | write(-)'
```

`inspect` catches type mismatches that are statically provable, such as field-based route stages whose source is known to be text rather than JSON.

Inspect a manifest that references a registered backend alias:

```bash
llmff inspect pipeline.yaml \
  --backend openai=https://api.openai.com/v1 \
  --api-key-env openai=OPENAI_API_KEY
```

List built-in stages:

```bash
llmff stages list
```

Use stdin/stdout by setting manifest input or output paths to `-`.

Inline graphs support linear `op`, `op(value)`, and `op(key=value)` stage syntax for `run` and `inspect`. Use semicolons inside the `documents` value for inline `retrieve`, such as `documents=docs/a.txt;docs/b.txt`. Manifests remain the format for branching graphs and version-controlled recipes.

For development without installing, prefix commands with `cargo run -p llmff --`, for example:

```bash
cargo run -p llmff -- inspect examples/json-repair.yaml
```

Manifest stages may be written in any order. `llmff` validates references across the full graph and executes stages in dependency order. By default stages execute sequentially for deterministic local behavior; `run --parallel` executes independent ready stages concurrently.

Manifest stages can reference file-backed resources relative to the manifest:

```yaml
graph:
  - id: render_prompt
    op: template
    from: load_prompt
    path: ./prompt.tmpl

  - id: apply_policy
    op: system
    from: render_prompt
    path: ./policy.md

  - id: validate
    op: validate_json
    from: draft
    schema_path: ./answer.schema.json
```

Inputs default to text. Set `format: json` to parse an input into a structured JSON value:

```yaml
inputs:
  payload:
    path: ./payload.json
    format: json
```

JSON inputs can be templated by object field and used by field-based routes. Invalid JSON fails the load stage with a stage execution error.

`template` replaces `{{input}}` when the parent value is text. When the parent value is a JSON object, object fields are available by name, such as `{{name}}`.

`system` with a `path` preserves chat roles for model calls: the file contents become a `system` message and the parent value becomes a `user` message. Text-only stages render messages conservatively as `role: content` lines when they need a string.

Retrieve stages select local UTF-8 documents with deterministic lexical scoring:

```yaml
graph:
  - id: retrieve_context
    op: retrieve
    from: load_prompt
    documents:
      - docs/python.txt
      - docs/rust.txt
    top_k: 1
```

`retrieve` renders its parent value as query text, tokenizes the query and each document, scores documents by matching unique query terms, sorts by score and path, and returns JSON with `query` and `matches`. Document paths are relative to the manifest unless absolute. `top_k` is optional and must be greater than zero when present.

Cache stages persist and reuse successful parent values across runs:

```yaml
graph:
  - id: cached_prompt
    op: cache
    from: render_prompt
    path: .llmff/cache
    key: prompt-v1
```

`cache` stores typed values as versioned JSON records under `path`, defaulting to `.llmff/cache`. With an explicit `key`, the manifest author controls the cache identity; without one, the parent value is part of the digest. Cache stages do not require environment variables and only cache successful parent values.

Route stages choose between already-computed stage outputs:

```yaml
graph:
  - id: choose_final
    op: route
    from: validate
    on_success: validate
    on_invalid: repair
```

For JSON object outputs, route can select by scalar field value:

```yaml
graph:
  - id: choose_model_output
    op: route
    from: classify
    field: kind
    cases:
      simple: fast_answer
      hard: strong_answer
    default: fast_answer
```

Stages can be guarded with `when` so they only run when their parent stage has a matching status:

```yaml
graph:
  - id: repair
    op: repair
    from: validate
    when: invalid
    model: mock:good
```

Supported conditions are `success`, `invalid`, and `skipped`. A non-matching condition marks the guarded stage as skipped before any stage-specific work runs, so model calls, tool calls, and writes are not invoked for skipped stages. Skipped stages still appear in traces with `status=skipped`.

Tool stages call explicitly declared commands or HTTP endpoints:

```yaml
graph:
  - id: normalize
    op: tool
    from: render_prompt
    command: ["/bin/cat"]
```

Command tools use argv directly, never a shell string. The serialized parent value is written to stdin and stdout becomes the stage output.

```yaml
graph:
  - id: call_endpoint
    op: tool
    from: render_prompt
    method: POST
    url: http://127.0.0.1:8080/process
    headers:
      content-type: text/plain
```

HTTP tools require `method` and `url`. `POST`, `PUT`, and `PATCH` send the serialized parent value as the request body; response text becomes the stage output.

Write stages persist a successful parent value from inside the graph and forward the same value:

```yaml
graph:
  - id: save_answer
    op: write
    from: validate
    path: ./answer.json
```

Top-level `outputs` remain supported for simple final outputs. Use `write` when the pipeline itself should express the write step, or when an intermediate value should be saved.

## Trace Notes

`--trace <path>` writes JSONL events for run and stage lifecycle events. Trace events include `timestamp_ms`; `stage_finished` events also include `duration_ms`.

Stage traces add safe operation metadata when available:

- `model`, `backend`, and `provider_model` for model-calling stages.
- `prompt_tokens`, `completion_tokens`, and `total_tokens` for model-calling stages when the backend reports usage.
- `validation_errors` for invalid validation results.
- `tool_kind` and `tool_target` for tool stages.
- `output_path` for write stages.
- `cache_hit` and `cache_path` for cache stages.

Trace metadata intentionally avoids full prompt bodies, cached values, tool stdin/stdout, headers, and secrets.

Summarize a trace file:

```bash
llmff trace /tmp/llmff-trace.jsonl
```

The trace summary prints run status, stage status, duration, and safe metadata only. Total usage is summarized as `usage=<total>`.

## Backend Notes

The CLI keeps backend registration explicit. This keeps commands portable and FFmpeg-like: the command line describes the run, while environment variables are only used when you choose to read a secret by name.

`run` and `inspect` accept the same backend registration flags. `inspect` validates that model ids resolve to configured backends, but it does not call model servers, tools, or pipeline stages.

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

Register a native Ollama backend:

```bash
llmff run pipeline.yaml \
  --ollama ollama=http://localhost:11434
```

Then reference the alias from a manifest or inline graph:

```yaml
graph:
  - id: draft
    op: infer
    from: load_prompt
    model: ollama:llama3.1
```

Model-calling stages accept portable sampling controls:

```yaml
graph:
  - id: draft
    op: infer
    from: load_prompt
    model: openai:gpt-test
    temperature: 0.2
    top_p: 0.9
    max_tokens: 256
```

OpenAI-compatible backends receive `temperature`, `top_p`, and `max_tokens`. Ollama receives the same controls under `options`, with `max_tokens` mapped to `num_predict`.

Mock backends remain available for deterministic local runs and tests:

- `LLMFF_MOCK_BAD_RESPONSE`
- `LLMFF_MOCK_GOOD_RESPONSE`

Those mock env vars are convenience fixtures, not the primary backend configuration model.

## Limitations

- Schema values are inline JSON strings in the current manifest format.
- Embedding, reranking, multimodal values, and plugin loading are not implemented yet.
- Native model loading, quantization, and hardware scheduling are out of scope for this MVP.
