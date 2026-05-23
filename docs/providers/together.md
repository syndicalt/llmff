# Together

Together exposes OpenAI-compatible chat-completion endpoints.

```bash
export TOGETHER_API_KEY='...'
llmff backends report --backend provider=https://api.together.xyz/v1 --api-key-env provider=TOGETHER_API_KEY
llmff run examples/providers/together.yaml --backend provider=https://api.together.xyz/v1 --api-key-env provider=TOGETHER_API_KEY
```

Compatibility: JSON mode, streaming, seed, stop, and usage metadata use
OpenAI-compatible fields. Check the selected Together model for strict JSON
behavior before using it without a validation stage.
