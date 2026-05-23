# OpenAI

OpenAI works through the OpenAI-compatible backend.

```bash
export OPENAI_API_KEY='...'
llmff backends report --backend provider=https://api.openai.com/v1 --api-key-env provider=OPENAI_API_KEY
llmff run examples/providers/openai.yaml --backend provider=https://api.openai.com/v1 --api-key-env provider=OPENAI_API_KEY
```

Compatibility: JSON mode is sent as `response_format: {"type":"json_object"}`;
streaming, seed, stop, and usage metadata are wired through the
OpenAI-compatible request and response shapes.
