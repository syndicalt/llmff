# vLLM

vLLM can serve an OpenAI-compatible API.

```bash
llmff backends report --backend provider=http://localhost:8000/v1
llmff run examples/providers/vllm.yaml --backend provider=http://localhost:8000/v1
```

Compatibility: JSON mode, streaming, seed, stop, and usage metadata are sent or
read through OpenAI-compatible fields. Confirm the served model supports guided
or JSON output before relying on JSON mode for strict production behavior.
