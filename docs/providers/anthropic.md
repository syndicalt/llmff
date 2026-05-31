# Anthropic

Anthropic is adapter-only in llmff today. The native Anthropic Messages API is
not implemented by the current backend code.

## Support Tier

Support tier: mock-inspectable

## Commands

Use Anthropic only through a gateway that exposes an OpenAI-compatible
`/v1/chat/completions` adapter, then register that gateway with `--backend`.

```bash
export ANTHROPIC_ADAPTER_API_KEY='...'
llmff backends report --backend provider=https://adapter.example.test/v1 --api-key-env provider=ANTHROPIC_ADAPTER_API_KEY
llmff run examples/providers/anthropic.yaml --backend provider=https://adapter.example.test/v1 --api-key-env provider=ANTHROPIC_ADAPTER_API_KEY
```

## Capabilities

Compatibility: JSON mode, streaming, seed, stop, and usage metadata are only as
honest as the adapter. If the adapter rejects an OpenAI-compatible field, remove
that field and keep validation or repair stages in the pipeline.

## Quirks

adapter behavior is the support boundary. Anthropic model ids in the manifest
are sent through the gateway, so provider drift can come from either the
adapter or Anthropic itself.

## Live Smoke

Live smoke: not configured

No repository live smoke is configured for this adapter path. Use the capability
report and the example manifest before advertising an adapter as supportable.
