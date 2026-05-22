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
