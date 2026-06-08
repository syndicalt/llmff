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

## External Composition Examples

`examples/wisepick-eventloom-flow/` shows how a caller can compose adjacent
runtime tools around `llmff` without changing `llmff` core:

```text
POST /v1/decide -> llmff run -> Eventloom-compatible JSONL -> POST /v1/feedback
```

The harness calls WisePick over HTTP, runs `llmff` as a subprocess, writes an
Eventloom-compatible JSONL journal, and sends WisePick feedback after execution.
It has a dry-run mode for offline validation:

```bash
python3 examples/wisepick-eventloom-flow/run.py \
  --dry-run \
  --intent "Clean and return this record as JSON" \
  --out-dir /tmp/llmff-wisepick-flow
```

## Manifest Templates

Production-ready workflow templates live in `examples/templates/`:

- `examples/templates/summarization.yaml`
- `examples/templates/structured-extraction.yaml`
- `examples/templates/classification.yaml`
- `examples/templates/json-repair.yaml`
- `examples/templates/self-refine-loop.yaml`
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

## Loop Examples

Bounded v1.1 loop examples live in `examples/loops/`:

- `examples/loops/self-refining-answer-loop.yaml`
- `examples/loops/react-style-tool-use-loop.yaml`
- `examples/loops/best-of-n-sampling+selection-loop.yaml`
- `examples/loops/iterative-research-fact-check-loop.yaml`

Start with the self-refining answer loop:

```bash
llmff inspect examples/loops/self-refining-answer-loop.yaml
LLMFF_MOCK_GOOD_RESPONSE='{"answer":"Use llmff for bounded, inspectable LLM pipelines.","confidence":0.93}' \
llmff run examples/loops/self-refining-answer-loop.yaml \
  --trace /tmp/llmff-self-refining-answer.trace.jsonl
```

The examples demonstrate `stage_success`, `field_true`, and `never` break
conditions while staying offline-friendly. See
[`examples/loops/README.md`](loops/README.md) for the full copy-run catalog and
real-provider adaptation notes.

## Real-World Workflows

Production-shaped examples live in `examples/real-world/`. They are offline mock
examples by default: each manifest uses deterministic `mock:*` model aliases, so
the commands are safe for local testing, CI checks, and agent-supervisor smoke
runs without provider credentials.

To adapt any workflow to a real provider, keep the same pipeline shape and change
the manifest model alias plus runtime backend registration. For example, replace
`model: mock:good` with a registered provider alias, then run with flags such as
`--backend openai=https://api.openai.com/v1 --api-key-env openai=OPENAI_API_KEY`
or `--ollama ollama=http://localhost:11434`.

For a production-shaped subprocess wrapper, run
[`examples/real-world/supervisor.py`](real-world/supervisor.py). It inspects the
issue-triage manifest, writes trace and event artifacts, preserves the `llmff`
exit code, and verifies the declared output artifact.

### Issue Triage

Use `examples/real-world/issue-triage.yaml` when you want to turn an inbound
support issue into a structured category, priority, summary, and next action.

Inspect:

```bash
llmff inspect examples/real-world/issue-triage.yaml
```

Run:

```bash
LLMFF_MOCK_GOOD_RESPONSE='{"category":"operations","priority":"high","summary":"Nightly invoice export times out before finance close.","recommended_action":"Escalate to the job owner, collect trace artifacts, and provide a same-day workaround."}' \
llmff run examples/real-world/issue-triage.yaml
```

Expected output artifact:

```text
examples/real-world/outputs/issue-triage.json
```

Cleanup:

```bash
rm -f examples/real-world/outputs/issue-triage.json
```

### Meeting Notes

Use `examples/real-world/meeting-notes.yaml` when you want meeting notes
summarized into a short recap, decisions, and owner-assigned action items.

Inspect:

```bash
llmff inspect examples/real-world/meeting-notes.yaml
```

Run:

```bash
LLMFF_MOCK_GOOD_RESPONSE='{"summary":"The team kept llmff focused on bounded execution and deferred package-manager publication.","decisions":["llmff remains an execution substrate, not an agent framework."],"actions":[{"owner":"Dana","task":"Draft production examples."},{"owner":"Ravi","task":"Review provider smoke expectations."}]}' \
llmff run examples/real-world/meeting-notes.yaml
```

Expected output artifact:

```text
examples/real-world/outputs/meeting-notes.json
```

Cleanup:

```bash
rm -f examples/real-world/outputs/meeting-notes.json
```

### Local RAG Answer

Use `examples/real-world/rag-answer.yaml` when you want to answer a question
from checked-in local documents using retrieval, reranking, and a final answer
step.

Inspect:

```bash
llmff inspect examples/real-world/rag-answer.yaml
```

Run:

```bash
LLMFF_MOCK_GOOD_RESPONSE='Use llmff as a bounded subprocess: inspect first, run with explicit artifacts, keep events and traces as metadata, and let the supervisor own retry policy.' \
llmff run examples/real-world/rag-answer.yaml
```

Expected output artifact:

```text
examples/real-world/outputs/rag-answer.txt
```

Cleanup:

```bash
rm -f examples/real-world/outputs/rag-answer.txt
```

### Batch Classification

Use `examples/real-world/batch-classification.yaml` when you want to classify
line-delimited work items and write isolated per-item batch results.

Inspect:

```bash
llmff inspect examples/real-world/batch-classification.yaml
```

Run:

```bash
LLMFF_MOCK_GOOD_RESPONSE='{"label":"support","confidence":0.91,"rationale":"The item asks for operational guidance."}' \
llmff run examples/real-world/batch-classification.yaml \
  --batch-input examples/real-world/inputs/batch-items.jsonl \
  --batch-output-dir "$PWD/examples/real-world/outputs/batch-items"
```

Expected output artifact:

```text
examples/real-world/outputs/batch-items/batch-report.jsonl
```

Cleanup:

```bash
rm -rf examples/real-world/outputs/batch-items/items \
  examples/real-world/outputs/batch-items/inputs \
  examples/real-world/outputs/batch-items/batch-report.jsonl
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

## Streaming And Supervision

See [`streaming-supervision.md`](streaming-supervision.md) for examples that
pipe lifecycle events into shell tools while keeping selected stage payloads on
a separate output stream.

For agent supervisors, see
[`../docs/agent-workflows.md`](../docs/agent-workflows.md) and the runnable
[`examples/agent-workflows/supervisor.py`](agent-workflows/supervisor.py)
subprocess example. Batch supervisors can use the offline
[`examples/agent-workflows/batch-supervisor.py`](agent-workflows/batch-supervisor.py)
example. JavaScript and TypeScript agent hosts can use the
streaming subprocess pattern in
[`examples/agent-workflows/node-supervisor.mjs`](agent-workflows/node-supervisor.mjs).

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
