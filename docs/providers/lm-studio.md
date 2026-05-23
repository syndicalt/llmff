# LM Studio

LM Studio exposes a local OpenAI-compatible server when its developer server is
enabled.

```bash
llmff backends report --backend provider=http://localhost:1234/v1
llmff run examples/providers/lm-studio.yaml --backend provider=http://localhost:1234/v1
```

Compatibility: JSON mode, streaming, seed, stop, and usage metadata are reported
as OpenAI-compatible request capabilities. Actual support varies by loaded local
model and LM Studio server version.
