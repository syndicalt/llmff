# Retrieve Stage Design

## Goal

Add the first retrieval hook to `llmff`: a deterministic, file-backed `retrieve` stage that selects relevant text documents for a query inside the pipeline graph.

## Rationale

The original pipeline-runner design explicitly lists retrieval hooks as an important graph operation after the core execution contract. The current graph can load, transform, call models, validate, route, call tools, write, and run independent stages in parallel, but it still has no built-in retrieval primitive. A small file-backed retriever gives users a real RAG building block while staying low-level and local.

## Behavior

- New stage operation: `retrieve`.
- Required fields:
  - `from`: parent query stage.
  - `documents`: non-empty list of file paths relative to the run working directory unless absolute.
- Optional field:
  - `top_k`: positive integer. Defaults to all documents.
- Input query can be `Text`, `Messages`, or `Json`; it is rendered into text using the same conservative representation as other text consumers.
- Each document is read as UTF-8 text.
- Scoring is deterministic lexical overlap:
  - tokenize query and document text into lowercase alphanumeric terms.
  - score is the count of query terms that appear in the document.
  - documents with score `0` are omitted.
  - ties are ordered by path for stable output.
- Output is `Value::Json`:

```json
{
  "query": "...",
  "matches": [
    {
      "path": "docs/a.txt",
      "score": 3,
      "text": "..."
    }
  ]
}
```

## Non-Goals

- No embedding model or vector database yet.
- No reranking stage yet.
- No chunking beyond whole-file documents.
- No remote retrieval plugins.
- No hidden file discovery or globbing in this slice.

## Verification

- Manifest parsing test covers `documents` and `top_k`.
- Engine validation rejects `retrieve` without `from` or without documents.
- Stage unit test proves lexical scoring, `top_k`, and stable ordering.
- CLI integration test proves a manifest can run retrieve and write JSON output.
- Full workspace tests and example inspect pass.
