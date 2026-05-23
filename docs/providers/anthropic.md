# Anthropic

Anthropic is adapter-only in llmff today. The native Anthropic Messages API is
not implemented by the current backend code.

Use Anthropic only through a gateway that exposes an OpenAI-compatible
`/v1/chat/completions` adapter, then register that gateway with `--backend`.

```bash
llmff backends report --backend provider=https://adapter.example.test/v1
```

Compatibility: JSON mode, streaming, seed, stop, and usage metadata are only as
honest as the adapter. If the adapter rejects an OpenAI-compatible field, remove
that field and keep validation or repair stages in the pipeline.
