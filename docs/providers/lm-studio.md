# LM Studio

LM Studio exposes a local OpenAI-compatible server when its developer server is
enabled.

## Support Tier

Support tier: Local documented

## Commands

```bash
llmff backends report --backend provider=http://localhost:1234/v1
llmff run examples/providers/lm-studio.yaml --backend provider=http://localhost:1234/v1
```

## Capabilities

Compatibility: JSON mode, streaming, seed, stop, and usage metadata are reported
as OpenAI-compatible request capabilities. Actual support varies by loaded local
model and LM Studio server version.

## Quirks

loaded local model support is the real compatibility boundary. Confirm the
server is running, the selected model is loaded, and the model accepts JSON mode
before treating a local run as representative.

## Live Smoke

Live smoke: not configured

No repository live smoke is configured for LM Studio because it depends on a
local desktop service and selected local model.
