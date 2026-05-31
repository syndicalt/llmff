# vLLM

vLLM can serve an OpenAI-compatible API.

## Support Tier

Support tier: mock-inspectable

## Commands

```bash
llmff backends report --backend provider=http://localhost:8000/v1
llmff run examples/providers/vllm.yaml --backend provider=http://localhost:8000/v1
```

## Capabilities

Compatibility: JSON mode, streaming, seed, stop, and usage metadata are sent or
read through OpenAI-compatible fields. Confirm the served model supports guided
or JSON output before relying on JSON mode for strict production behavior.

## Quirks

served model configuration is the compatibility boundary. Confirm the vLLM
server, served model id, guided decoding options, and usage metadata behavior
before treating the endpoint as supportable.

## Live Smoke

Live smoke: not configured

No repository live smoke is configured for vLLM because setup depends on local
or self-hosted model serving capacity.
