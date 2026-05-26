# Azure OpenAI

Azure OpenAI is usable only through an OpenAI-compatible chat-completions route
or gateway URL. Register the deployment URL as an OpenAI-compatible backend.

## Support Tier

Support tier: Documented gateway

## Commands

```bash
export AZURE_OPENAI_API_KEY='...'
llmff backends report --backend provider=https://example.openai.azure.com/openai/deployments/deployment-id --api-key-env provider=AZURE_OPENAI_API_KEY
llmff run examples/providers/azure-openai.yaml --backend provider=https://example.openai.azure.com/openai/deployments/deployment-id --api-key-env provider=AZURE_OPENAI_API_KEY
```

## Capabilities

Compatibility: JSON mode, streaming, seed, stop, and usage metadata depend on
the Azure deployment and API-version gateway exposing OpenAI-compatible chat
completions. If Azure rejects a field, remove that field from the manifest and
keep `validate_json` in the pipeline.

## Quirks

deployment URLs and API-version routing are the main compatibility boundary.
Confirm the selected deployment accepts the same request fields as the manifest
before relying on the example in production.

## Live Smoke

Live smoke: not configured

No repository live smoke is configured for Azure OpenAI. Use the capability
report and the example manifest before promoting a deployment.
