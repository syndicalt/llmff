# OpenRouter

OpenRouter is an OpenAI-compatible gateway with provider-qualified model ids.

## Support Tier

Support tier: Documented gateway

## Commands

```bash
export OPENROUTER_API_KEY='...'
llmff backends report --backend provider=https://openrouter.ai/api/v1 --api-key-env provider=OPENROUTER_API_KEY
llmff run examples/providers/openrouter.yaml --backend provider=https://openrouter.ai/api/v1 --api-key-env provider=OPENROUTER_API_KEY
```

## Capabilities

Compatibility: JSON mode, streaming, seed, stop, and usage metadata are wired as
OpenAI-compatible fields. Support can vary by routed upstream model.

## Quirks

routed upstream model behavior is the main compatibility boundary. Confirm the
selected upstream model accepts JSON mode and sampling fields before relying on
the provider-level endpoint behavior.

## Live Smoke

Live smoke: not configured

No repository live smoke is configured for OpenRouter. Use the capability
report and the example manifest before promoting a route.
