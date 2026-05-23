# LocalAI

LocalAI can expose OpenAI-compatible chat completions for local models.

```bash
llmff backends report --backend provider=http://localhost:8080/v1
llmff run examples/providers/localai.yaml --backend provider=http://localhost:8080/v1
```

Compatibility: JSON mode, streaming, seed, stop, and usage metadata are treated
as OpenAI-compatible fields. LocalAI model backends differ, so use
`validate_json` and remove rejected request fields if a model returns 400.
