# Groq

Groq exposes OpenAI-compatible chat-completion endpoints.

## Support Tier

Support tier: mock-inspectable

## Commands

```bash
export GROQ_API_KEY='...'
llmff backends report --backend provider=https://api.groq.com/openai/v1 --api-key-env provider=GROQ_API_KEY
llmff run examples/providers/groq.yaml --backend provider=https://api.groq.com/openai/v1 --api-key-env provider=GROQ_API_KEY
```

## Capabilities

Compatibility: JSON mode, streaming, seed, stop, and usage metadata are sent or
read through OpenAI-compatible fields. Model-level support may differ.

## Quirks

model-level behavior can differ even when the OpenAI-compatible endpoint
accepts the request. If a selected model rejects JSON mode or sampling fields,
remove the least portable field first and keep `validate_json`.

## Live Smoke

Live smoke: not configured

No repository live smoke is configured for Groq. Use the capability report and
the example manifest before promoting a model.
