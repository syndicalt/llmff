# Provider Support Tiers

Provider support in llmff is explicit and evidence-backed. A provider is not
treated as broadly supported just because an OpenAI-compatible endpoint exists.
Each provider page documents the tier, capabilities, quirks, runnable manifest,
and live smoke posture.

## Tier Definitions

- documented only: documentation exists, but there is no repository-owned
  example manifest or automated readiness evidence yet.
- mock-inspectable: documentation and an example manifest exist, and the
  manifest can be inspected with deterministic local or placeholder provider
  registration. This is not live provider certification.
- opt-in smoke ready: documentation, an example manifest, and an explicit
  opt-in smoke path exist. The smoke is gated by `workflow_dispatch` or
  `LLMFF_LIVE_PROVIDER_SMOKE=1` and does not run on pull requests or pushes.
- live-smoke verified: maintainers have run the opt-in smoke against a real
  endpoint and recorded a current successful result in
  `docs/providers/live-smoke-history.json`.

As of the current release line, no provider is live-smoke verified. The
OpenAI-compatible and Ollama paths are opt-in smoke ready; the other provider
pages are mock-inspectable unless promoted with new evidence.

## Provider Matrix

| Provider | Support tier | Live smoke | Evidence |
| --- | --- | --- | --- |
| `anthropic` | mock-inspectable | not configured | Adapter-only guidance, example manifest, and capability report command. |
| `azure-openai` | mock-inspectable | not configured | Gateway guidance, example manifest, deployment caveats, and capability report command. |
| `groq` | mock-inspectable | not configured | Gateway guidance, example manifest, model caveats, and capability report command. |
| `lm-studio` | mock-inspectable | not configured | Local server guidance, example manifest, loaded-model caveats, and capability report command. |
| `localai` | mock-inspectable | not configured | Local server guidance, example manifest, backend caveats, and capability report command. |
| `ollama` | opt-in smoke ready | workflow_dispatch | Workflow-dispatch smoke, local service guidance, example manifest, and readiness history. |
| `openai` | mock-inspectable | via openai-compatible | OpenAI-compatible backend guidance, example manifest, and capability report command. |
| `openai-compatible` | opt-in smoke ready | workflow_dispatch | Workflow-dispatch smoke, script opt-in gate, example manifest, and readiness history. |
| `openrouter` | mock-inspectable | not configured | Gateway guidance, example manifest, routed-model caveats, and capability report command. |
| `together` | mock-inspectable | not configured | Gateway guidance, example manifest, strict JSON caveats, and capability report command. |
| `vllm` | mock-inspectable | not configured | Local server guidance, example manifest, served-model caveats, and capability report command. |
