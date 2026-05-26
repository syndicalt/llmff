# OpenAI-Compatible

OpenAI-compatible endpoints are the primary hosted provider integration shape
for llmff. Register the endpoint root with `--backend`; the backend appends the
chat completions path.

## Support Tier

Support tier: Opt-in live smoke

## Commands

```bash
export OPENAI_API_KEY='...'
export OPENAI_BASE_URL='https://api.openai.com/v1'
llmff backends report --backend openai="$OPENAI_BASE_URL" --api-key-env openai=OPENAI_API_KEY
llmff run examples/providers/openai-compatible.yaml --backend openai="$OPENAI_BASE_URL" --api-key-env openai=OPENAI_API_KEY
```

## Capabilities

Compatibility: JSON mode is sent as `response_format:
{"type":"json_object"}`; streaming uses server-sent events when the endpoint
supports them; seed and stop are sent as OpenAI-compatible request fields; usage
metadata is read from OpenAI-compatible response usage fields.

## Quirks

base URL normalization adds `/v1` when the registered endpoint omits it. Do not
include `/chat/completions` in the backend URL. Some compatible gateways reject
OpenAI request fields even when they expose the same route; remove rejected
fields and keep `validate_json` in the pipeline.

## Live Smoke

Live smoke: workflow_dispatch

The opt-in smoke path is `.github/workflows/live-provider-smoke.yml` plus
`scripts/smoke-openai-compatible-provider.sh`. It requires
`LLMFF_LIVE_PROVIDER_SMOKE=1`, `OPENAI_API_KEY`, and optional
`OPENAI_BASE_URL`.
