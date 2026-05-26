# Provider Support Tiers

Provider support in llmff is explicit and evidence-backed. A provider is not
treated as broadly supported just because an OpenAI-compatible endpoint exists.
Each provider page documents the tier, capabilities, quirks, runnable manifest,
and live smoke posture.

## Tier Definitions

- Opt-in live smoke: the provider path has an opt-in `workflow_dispatch` smoke
  job or script with documented runner, secret, and readiness expectations.
- Documented gateway: the provider has a maintained example and capability
  guidance, but no dedicated live smoke job in this repository.
- Documented adapter: the provider is supported only through an
  OpenAI-compatible adapter or gateway, with adapter drift called out.
- Local documented: the provider path targets a local server and is documented
  with local setup assumptions instead of hosted-provider certification.

## Provider Matrix

| Provider | Support tier | Live smoke | Evidence |
| --- | --- | --- | --- |
| `anthropic` | Documented adapter | not configured | Adapter-only guidance, example manifest, and capability report command. |
| `azure-openai` | Documented gateway | not configured | Gateway guidance, example manifest, deployment caveats, and capability report command. |
| `groq` | Documented gateway | not configured | Gateway guidance, example manifest, model caveats, and capability report command. |
| `lm-studio` | Local documented | not configured | Local server guidance, example manifest, loaded-model caveats, and capability report command. |
| `localai` | Local documented | not configured | Local server guidance, example manifest, backend caveats, and capability report command. |
| `ollama` | Opt-in live smoke | workflow_dispatch | Workflow-dispatch smoke, local service guidance, example manifest, and readiness history. |
| `openai` | Documented gateway | via openai-compatible | OpenAI-compatible backend guidance, example manifest, and capability report command. |
| `openai-compatible` | Opt-in live smoke | workflow_dispatch | Workflow-dispatch smoke, script opt-in gate, example manifest, and readiness history. |
| `openrouter` | Documented gateway | not configured | Gateway guidance, example manifest, routed-model caveats, and capability report command. |
| `together` | Documented gateway | not configured | Gateway guidance, example manifest, strict JSON caveats, and capability report command. |
| `vllm` | Local documented | not configured | Local server guidance, example manifest, served-model caveats, and capability report command. |
