# Examples

The examples are small, deterministic fixtures that can be run without provider
credentials. They are intended for first-run smoke tests and for copying into
new pipeline manifests.

## JSON Repair

Files:

- `json-repair.yaml`: pipeline manifest.
- `question.txt`: user prompt input.
- `prompt.tmpl`: prompt template.
- `policy.md`: system policy text.
- `answer.schema.json`: output schema.

Run:

```bash
LLMFF_MOCK_BAD_RESPONSE='{"wrong":true}' \
LLMFF_MOCK_GOOD_RESPONSE='{"answer":"ok"}' \
llmff run examples/json-repair.yaml --trace /tmp/llmff-trace.jsonl
```

Output:

```text
examples/answer.json
```

Expected contents:

```json
{"answer":"ok"}
```

Clean up generated output:

```bash
rm -f examples/answer.json /tmp/llmff-trace.jsonl
```

## What The Pipeline Demonstrates

The manifest shows the current core workflow shape:

- `load` reads file-backed input.
- `template` renders prompt text with `{{input}}`.
- `system` turns a policy file into a system message.
- `infer` calls a configured model backend.
- `validate_json` checks model output against a JSON Schema file.
- `repair` calls another model when validation fails.
- `route` chooses the valid draft or repaired output.
- Top-level `outputs` writes the final value to disk.

## Adapting It

To call a real OpenAI-compatible backend:

1. Change `model: mock:bad` and `model: mock:good` to a registered alias, such
   as `model: openai:gpt-4.1-mini`.
2. Run with an explicit backend registration:

```bash
export OPENAI_API_KEY='...'

llmff run examples/json-repair.yaml \
  --backend openai=https://api.openai.com/v1 \
  --api-key-env openai=OPENAI_API_KEY
```

To use Ollama, set the manifest model to an Ollama alias such as
`ollama:llama3.1`, then run:

```bash
llmff run examples/json-repair.yaml \
  --ollama ollama=http://localhost:11434
```

## Provider Examples

OpenAI-compatible and Ollama manifests live in `examples/providers/`. Each real
provider manifest has a mock fallback that runs without network access.

OpenAI-compatible:

```bash
export OPENAI_API_KEY='...'
export OPENAI_BASE_URL='https://api.openai.com/v1'

llmff run examples/providers/openai-compatible.yaml \
  --backend openai="$OPENAI_BASE_URL" \
  --api-key-env openai=OPENAI_API_KEY
```

Mock fallback:

```bash
LLMFF_MOCK_GOOD_RESPONSE='{"answer":"ok"}' \
llmff run examples/providers/openai-compatible.mock.yaml
```

Ollama:

```bash
export OLLAMA_BASE_URL='http://localhost:11434'

llmff run examples/providers/ollama.yaml \
  --ollama ollama="$OLLAMA_BASE_URL"
```

Mock fallback:

```bash
LLMFF_MOCK_GOOD_RESPONSE='{"answer":"ok"}' \
llmff run examples/providers/ollama.mock.yaml
```

Provider setup and failure handling are documented in
[`docs/provider-troubleshooting.md`](../docs/provider-troubleshooting.md).

Optional live smoke scripts are available for provider onboarding. They skip by
default and only call real endpoints when explicitly enabled:

```bash
scripts/smoke-openai-compatible-provider.sh
scripts/smoke-ollama-provider.sh
```

Run the OpenAI-compatible smoke against a live endpoint:

```bash
export LLMFF_LIVE_PROVIDER_SMOKE=1
export OPENAI_API_KEY='...'
export OPENAI_BASE_URL='https://api.openai.com/v1'

scripts/smoke-openai-compatible-provider.sh
```

Run the Ollama smoke against a local service:

```bash
export LLMFF_LIVE_PROVIDER_SMOKE=1
export OLLAMA_BASE_URL='http://localhost:11434'

scripts/smoke-ollama-provider.sh
```

## Manifest Templates

Production-ready workflow templates live in `examples/templates/`:

- `examples/templates/summarization.yaml`
- `examples/templates/structured-extraction.yaml`
- `examples/templates/classification.yaml`
- `examples/templates/json-repair.yaml`
- `examples/templates/rag-answer.yaml`
- `examples/templates/batch-processing.yaml`
- `examples/templates/tool-calling.yaml`
- `examples/templates/eval-harness.yaml`
- `examples/templates/multi-provider-fallback.yaml`
- `examples/templates/cost-latency-comparison.yaml`

Inspect any template before adapting it:

```bash
llmff inspect examples/templates/structured-extraction.yaml
```

Copy-this-and-run-it commands for every template are documented in
[`docs/pipeline-library.md`](../docs/pipeline-library.md). The fallback and
cost/latency examples are simulations built from currently available stages;
the pipeline library doc explains exactly what they do.

## Inline Smoke Examples

Run a one-line mock pipeline:

```bash
LLMFF_MOCK_GOOD_RESPONSE='{"answer":"ok"}' \
llmff run -i examples/question.txt \
  -g 'load | infer(model=mock:good) | write(-)'
```

Inspect a one-line pipeline without model calls:

```bash
llmff inspect -g 'load | infer(model=mock:good) | write(-)'
```

## Streaming And Supervision

See [`streaming-supervision.md`](streaming-supervision.md) for examples that
pipe lifecycle events into shell tools while keeping selected stage payloads on
a separate output stream.

## Retrieval Fixtures

Files:

- `retrieval/python.txt`
- `retrieval/rust.txt`

Run local retrieval without a model backend:

```bash
llmff run -i examples/question.txt \
  -g 'load | retrieve(documents=examples/retrieval/python.txt;examples/retrieval/rust.txt,top_k=1) | write(-)'
```

Run retrieval plus local reranking:

```bash
llmff run -i examples/question.txt \
  -g 'load | retrieve(documents=examples/retrieval/python.txt;examples/retrieval/rust.txt,top_k=2) | rerank(strategy=embedding,top_k=1) | write(-)'
```
