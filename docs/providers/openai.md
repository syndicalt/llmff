# OpenAI

OpenAI works through the OpenAI-compatible backend.

## Support Tier

Support tier: Documented gateway

## Commands

```bash
export OPENAI_API_KEY='...'
llmff backends report --backend provider=https://api.openai.com/v1 --api-key-env provider=OPENAI_API_KEY
llmff run examples/providers/openai.yaml --backend provider=https://api.openai.com/v1 --api-key-env provider=OPENAI_API_KEY
```

## Capabilities

Compatibility: JSON mode is sent as `response_format: {"type":"json_object"}`;
streaming, seed, stop, and usage metadata are wired through the
OpenAI-compatible request and response shapes.

## Quirks

response_format support still depends on the selected model. If the model or
account rejects structured-output fields, remove `response_format: json` and
keep `validate_json` or repair stages in the manifest.

## Live Smoke

Live smoke: via openai-compatible

OpenAI is covered by the OpenAI-compatible smoke path when `OPENAI_BASE_URL`
points at `https://api.openai.com/v1`.
