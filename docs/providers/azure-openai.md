# Azure OpenAI

Azure OpenAI is usable only through an OpenAI-compatible chat-completions route
or gateway URL. Register the deployment URL as an OpenAI-compatible backend.

```bash
export AZURE_OPENAI_API_KEY='...'
llmff backends report --backend provider=https://example.openai.azure.com/openai/deployments/deployment-id --api-key-env provider=AZURE_OPENAI_API_KEY
llmff run examples/providers/azure-openai.yaml --backend provider=https://example.openai.azure.com/openai/deployments/deployment-id --api-key-env provider=AZURE_OPENAI_API_KEY
```

Compatibility: JSON mode, streaming, seed, stop, and usage metadata depend on
the Azure deployment and API-version gateway exposing OpenAI-compatible chat
completions. If Azure rejects a field, remove that field from the manifest and
keep `validate_json` in the pipeline.
