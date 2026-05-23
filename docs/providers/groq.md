# Groq

Groq exposes OpenAI-compatible chat-completion endpoints.

```bash
export GROQ_API_KEY='...'
llmff backends report --backend provider=https://api.groq.com/openai/v1 --api-key-env provider=GROQ_API_KEY
llmff run examples/providers/groq.yaml --backend provider=https://api.groq.com/openai/v1 --api-key-env provider=GROQ_API_KEY
```

Compatibility: JSON mode, streaming, seed, stop, and usage metadata are sent or
read through OpenAI-compatible fields. Model-level support may differ.
