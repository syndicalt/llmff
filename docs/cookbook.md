# Cookbook

This cookbook routes common patterns to existing examples. The examples are
offline-runnable by default with deterministic mock backends, so they are safe
for local smoke tests and CI. This page is not a CLI reference; use it to pick a
pipeline shape, then adapt the linked manifest.

## RAG-Lite Answering

Start with local file retrieval and reranking:

- Template: `examples/templates/rag-answer.yaml`
- Real-world shape: `examples/real-world/rag-answer.yaml`
- Reference docs: [`docs/pipeline-library.md`](pipeline-library.md)

Run the template with:

```bash
llmff inspect examples/templates/rag-answer.yaml
LLMFF_MOCK_GOOD_RESPONSE='The local context says Rust and Python are available.' \
llmff run examples/templates/rag-answer.yaml
```

## Structured Extraction

Use a schema-validated output when downstream code needs predictable JSON:

- Template: `examples/templates/structured-extraction.yaml`
- Related repair flow: `examples/templates/json-repair.yaml`
- First-run example: `examples/json-repair.yaml`

## Tool Chains

Use tool stages when an explicit command or HTTP tool should run before model
synthesis:

- Template: `examples/templates/tool-calling.yaml`
- Plugin transport examples: `examples/plugins/`
- Plugin docs: [`docs/plugins.md`](plugins.md)

## Evaluation Pipelines

Use the eval harness shape when a candidate answer must validate against a
known schema:

- Template: `examples/templates/eval-harness.yaml`
- Trace and metrics docs: [`docs/observability.md`](observability.md)

## Batch Processing

Use batch mode when the caller has isolated input items and wants per-item
outputs plus a batch report:

- Template: `examples/templates/batch-processing.yaml`
- Real-world shape: `examples/real-world/batch-classification.yaml`
- Supervisor example: `examples/agent-workflows/batch-supervisor.py`

Keep batch input and output directories explicit, and use `--run-dir` for
run-scoped inspect, trace, events, checkpoint, and result artifacts.

## Agent Supervisor Integration

Use `llmff` as a bounded subprocess inside the agent host:

- Python supervisor: `examples/agent-workflows/supervisor.py`
- Batch supervisor: `examples/agent-workflows/batch-supervisor.py`
- Node.js streaming supervisor: `examples/agent-workflows/node-supervisor.mjs`
- Contract docs: [`docs/agent-workflows.md`](agent-workflows.md)

The canonical pattern is inspect, run, preserve the original exit code, store
the run-directory artifacts, and read safe failure kinds from `run_failed` or
`result.json` after a non-zero exit.

## Issue Triage And Operations

Use the real-world fixtures when you want production-shaped manifests without
live provider credentials:

- `examples/real-world/issue-triage.yaml`
- `examples/real-world/meeting-notes.yaml`
- `examples/real-world/rag-answer.yaml`
- `examples/real-world/batch-classification.yaml`

These examples are documented in [`examples/README.md`](../examples/README.md)
and are covered by the example catalog tests.
