# Provider Troubleshooting

Use this page when an OpenAI-compatible or Ollama run fails before the model
returns useful text.

## API key lookup

OpenAI-compatible backends need an API key unless the server explicitly allows
anonymous requests.

```bash
export OPENAI_API_KEY='...'

llmff run examples/providers/openai-compatible.yaml \
  --backend openai=https://api.openai.com/v1 \
  --api-key-env openai=OPENAI_API_KEY
```

The alias on `--api-key-env` must match the alias on `--backend`. For example,
`--backend openai=...` pairs with `--api-key-env openai=OPENAI_API_KEY`.

For local test servers or gateways that do not require a key, omit
`--api-key-env`. For one-off local smoke checks, `--api-key openai=test-key`
registers a literal key without reading the environment.

Ollama does not use API keys:

```bash
llmff run examples/providers/ollama.yaml \
  --ollama ollama=http://localhost:11434
```

Provider example environment variables:

| Variable | Used by | Purpose |
| --- | --- | --- |
| `OPENAI_API_KEY` | OpenAI-compatible | Secret read by `--api-key-env openai=OPENAI_API_KEY`. |
| `OPENAI_BASE_URL` | OpenAI-compatible | Optional shell variable for the value passed to `--backend openai=...`. |
| `OLLAMA_BASE_URL` | Ollama | Optional shell variable for the value passed to `--ollama ollama=...`. |
| `LLMFF_MOCK_GOOD_RESPONSE` | Mock fallbacks | Deterministic model response for `.mock.yaml` examples. |
| `LLMFF_MOCK_BAD_RESPONSE` | JSON repair mocks | Deterministic invalid draft for repair workflows. |

## base URL normalization

OpenAI-compatible base URLs are normalized to include `/v1`.

- `https://api.openai.com` becomes `https://api.openai.com/v1`.
- `https://api.openai.com/v1` stays unchanged.
- A trailing slash is removed before normalization.

Register the provider root or the versioned root, not the full endpoint. The
backend appends `/chat/completions`.

```bash
llmff run examples/providers/openai-compatible.yaml \
  --backend openai=https://api.openai.com \
  --api-key-env openai=OPENAI_API_KEY
```

Ollama base URLs only trim trailing slashes. The backend appends `/api/chat`.

## JSON response-format support

Set `response_format: json` on `infer` and `repair` stages when the selected
model should return a JSON object.

```yaml
- id: draft
  op: infer
  from: apply_policy
  model: openai:gpt-4.1-mini
  response_format: json
```

OpenAI-compatible backends send this as `response_format: {"type":"json_object"}`.
Ollama sends it as `format: "json"`.

Some provider-compatible gateways or older local models ignore or reject JSON
format options. If a run fails with a 400-level response, remove
`response_format: json`, keep a strict system prompt, and validate the output
with `validate_json` plus a `repair` stage.

## token streaming support

OpenAI-compatible backends support streaming inference with `--stream-stage`.
Use a stage id that produces model text.

```bash
llmff run examples/providers/openai-compatible.yaml \
  --backend openai=https://api.openai.com/v1 \
  --api-key-env openai=OPENAI_API_KEY \
  --stream-stage draft
```

Streaming writes the selected model stage as it arrives and still records usage
metadata when the provider reports it.

Ollama currently runs through the non-streaming chat path in `llmff`. If you need
streaming output, use an OpenAI-compatible endpoint that supports server-sent
events, or run Ollama without `--stream-stage`.

## common HTTP failure modes

- `401 Unauthorized`: the key is missing, expired, or registered under the wrong
  backend alias. Check `--api-key-env alias=ENV_NAME` and `echo "$ENV_NAME"`.
- `403 Forbidden`: the key is valid but lacks access to the model or project.
  Choose an enabled model or update provider permissions.
- `404 Not Found`: the base URL or model id is wrong. Do not include
  `/chat/completions` in `--backend`; use model ids such as
  `openai:gpt-4.1-mini`.
- `429 Too Many Requests`: provider rate limit or quota exhaustion. Retry later,
  reduce parallelism, or choose a model with available quota.
- `400 Bad Request`: unsupported request fields, often `response_format`,
  `seed`, `stop`, or sampling parameters. Remove the least portable option first.
- `5xx`: provider or local server failure. Retry once, then check server logs.
- `request failed`: DNS, TLS, proxy, firewall, or server availability issue.
  Confirm the host with `curl` and check corporate proxy settings.

## Offline mock fallbacks

Each provider example has a mock fallback that validates the same pipeline shape
without network access:

```bash
LLMFF_MOCK_GOOD_RESPONSE='{"answer":"ok"}' \
llmff run examples/providers/openai-compatible.mock.yaml

LLMFF_MOCK_GOOD_RESPONSE='{"answer":"ok"}' \
llmff run examples/providers/ollama.mock.yaml
```
