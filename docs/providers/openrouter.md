# OpenRouter

OpenRouter is an OpenAI-compatible gateway with provider-qualified model ids.

```bash
export OPENROUTER_API_KEY='...'
llmff backends report --backend provider=https://openrouter.ai/api/v1 --api-key-env provider=OPENROUTER_API_KEY
llmff run examples/providers/openrouter.yaml --backend provider=https://openrouter.ai/api/v1 --api-key-env provider=OPENROUTER_API_KEY
```

Compatibility: JSON mode, streaming, seed, stop, and usage metadata are wired as
OpenAI-compatible fields. Support can vary by routed upstream model.
