# Together

Together exposes OpenAI-compatible chat-completion endpoints.

## Support Tier

Support tier: mock-inspectable

## Commands

```bash
export TOGETHER_API_KEY='...'
llmff backends report --backend provider=https://api.together.xyz/v1 --api-key-env provider=TOGETHER_API_KEY
llmff run examples/providers/together.yaml --backend provider=https://api.together.xyz/v1 --api-key-env provider=TOGETHER_API_KEY
```

## Capabilities

Compatibility: JSON mode, streaming, seed, stop, and usage metadata use
OpenAI-compatible fields. Check the selected Together model for strict JSON
behavior before using it without a validation stage.

## Quirks

strict JSON behavior varies by model. Keep `validate_json` in the manifest and
remove unsupported request fields if the provider returns a 400-level response.

## Live Smoke

Live smoke: not configured

No repository live smoke is configured for Together. Use the capability report
and the example manifest before promoting a model.
