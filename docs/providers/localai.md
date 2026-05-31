# LocalAI

LocalAI can expose OpenAI-compatible chat completions for local models.

## Support Tier

Support tier: mock-inspectable

## Commands

```bash
llmff backends report --backend provider=http://localhost:8080/v1
llmff run examples/providers/localai.yaml --backend provider=http://localhost:8080/v1
```

## Capabilities

Compatibility: JSON mode, streaming, seed, stop, and usage metadata are treated
as OpenAI-compatible fields. LocalAI model backends differ, so use
`validate_json` and remove rejected request fields if a model returns 400.

## Quirks

model backends differ across LocalAI configurations. Keep the manifest shape
stable, but validate each served model before relying on JSON mode, seed, stop,
or usage metadata.

## Live Smoke

Live smoke: not configured

No repository live smoke is configured for LocalAI because setup depends on the
local server image, model backend, and model files.
