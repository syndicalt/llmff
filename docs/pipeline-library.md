# Pipeline Library

These templates are deterministic, offline-friendly starting points. They use
the mock backend by default so `llmff inspect` and `llmff run` work without
provider credentials. Replace `mock:*` model aliases with real provider aliases
when adapting a template for production.

Each example writes output under `examples/templates/`. Remove generated files
with `rm -f examples/templates/*.answer.* examples/templates/*.result.json`.

## Summarization

Use this when a text input should become a concise text answer.

```bash
llmff inspect examples/templates/summarization.yaml
LLMFF_MOCK_GOOD_RESPONSE='Short summary.' \
llmff run examples/templates/summarization.yaml
```

## Extraction

Use this when model output must match a JSON object schema.

```bash
llmff inspect examples/templates/structured-extraction.yaml
LLMFF_MOCK_GOOD_RESPONSE='{"answer":"ok"}' \
llmff run examples/templates/structured-extraction.yaml
```

## Classification

Use this for single-label classification with a numeric confidence.

```bash
llmff inspect examples/templates/classification.yaml
LLMFF_MOCK_GOOD_RESPONSE='{"label":"reference","confidence":0.91}' \
llmff run examples/templates/classification.yaml
```

## JSON Repair

Use this when a first draft may violate a schema and should be repaired before
the final route.

```bash
llmff inspect examples/templates/json-repair.yaml
LLMFF_MOCK_BAD_RESPONSE='{"wrong":true}' \
LLMFF_MOCK_GOOD_RESPONSE='{"answer":"ok"}' \
llmff run examples/templates/json-repair.yaml
```

## RAG Answer

Use this for local file-backed retrieval, lexical reranking, and answer
generation.

```bash
llmff inspect examples/templates/rag-answer.yaml
LLMFF_MOCK_GOOD_RESPONSE='The local context says Rust and Python are available.' \
llmff run examples/templates/rag-answer.yaml
```

## Batch Processing

Use this shape for a batch request that retrieves context, reranks it, asks a
model for structured output, and validates the result.

```bash
llmff inspect examples/templates/batch-processing.yaml
LLMFF_MOCK_GOOD_RESPONSE='{"answer":"ok"}' \
llmff run examples/templates/batch-processing.yaml
```

## Tool Calling

Use this for an explicit local command tool call before model synthesis. The
template uses `/bin/cat` as the deterministic tool transport.

```bash
llmff inspect examples/templates/tool-calling.yaml
LLMFF_MOCK_GOOD_RESPONSE='Tool result accepted.' \
llmff run examples/templates/tool-calling.yaml
```

## Eval Harness

Use this for an offline eval case where the candidate answer must validate
against a known schema.

```bash
llmff inspect examples/templates/eval-harness.yaml
LLMFF_MOCK_GOOD_RESPONSE='{"answer":"ok"}' \
llmff run examples/templates/eval-harness.yaml
```

## Multi-Provider Fallback

Multi-provider fallback is simulated with available stages: `mock:bad` produces
an invalid primary JSON draft, `validate_json` marks it invalid, `repair` calls
`mock:good`, and `route` selects the repaired value. In production, replace the
primary and fallback model aliases with different provider-backed aliases.

```bash
llmff inspect examples/templates/multi-provider-fallback.yaml
LLMFF_MOCK_BAD_RESPONSE='{"wrong":true}' \
LLMFF_MOCK_GOOD_RESPONSE='{"answer":"ok"}' \
llmff run examples/templates/multi-provider-fallback.yaml
```

## Cost/Latency Comparison

Cost/latency comparison is simulated with available stages: the template fans
out to two mock model candidates with different sampling budgets and routes the
fast candidate as the final answer. It does not calculate provider cost. Use
`--trace` with real providers to collect stage durations and token metadata,
then compare those trace fields outside the pipeline.

```bash
llmff inspect examples/templates/cost-latency-comparison.yaml
LLMFF_MOCK_GOOD_RESPONSE='Fast candidate.' \
llmff run examples/templates/cost-latency-comparison.yaml --trace /tmp/llmff-cost-latency.jsonl
```
