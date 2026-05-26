# Ollama

Ollama uses the native Ollama chat backend and a local service URL registered
with `--ollama`.

## Support Tier

Support tier: Opt-in live smoke

## Commands

```bash
export OLLAMA_BASE_URL='http://localhost:11434'
llmff backends report --ollama ollama="$OLLAMA_BASE_URL"
llmff run examples/providers/ollama.yaml --ollama ollama="$OLLAMA_BASE_URL"
```

## Capabilities

Compatibility: JSON mode is sent as `format: "json"`; streaming is
non-streaming in the current llmff Ollama path; seed and stop support depends
on the served Ollama model and runtime; usage metadata is not guaranteed across
local model responses.

## Quirks

non-streaming execution means `--stream-stage` should use an OpenAI-compatible
endpoint instead. Model size, download time, disk, memory, and local service
health are part of the support surface for live smoke readiness.

## Live Smoke

Live smoke: workflow_dispatch

The opt-in smoke path is `.github/workflows/live-provider-smoke.yml` plus
`scripts/smoke-ollama-provider.sh`. It requires `LLMFF_LIVE_PROVIDER_SMOKE=1`
and `OLLAMA_BASE_URL`; GitHub Actions setup installs Ollama and pulls
`llama3.1` explicitly.
