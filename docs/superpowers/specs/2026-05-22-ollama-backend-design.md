# Ollama Backend Design

## Goal

Add a shallow native Ollama backend adapter so manifests can use model ids such as:

```yaml
model: ollama:llama3.1
```

## Source

The official Ollama API documentation describes `POST /api/chat` as the endpoint for generating the next chat message. The request body includes `model` and `messages`; `stream` defaults to true, so this adapter sends `stream: false` for a single JSON response.

## CLI Shape

```bash
llmff run pipeline.yaml \
  --ollama ollama=http://localhost:11434
```

The alias before `=` is the backend alias used in manifest model ids. The value is the Ollama base URL.

## Semantics

- `OllamaBackend` implements the existing `Backend` trait.
- It posts to `{base_url}/api/chat`.
- It sends:
  - `model`: provider model id.
  - `messages`: one user message containing the prompt.
  - `stream`: false.
  - `options.temperature` only when a temperature is present.
- It returns `message.content` as `InferResponse.text`.
- Non-2xx responses are backend errors containing status and response body.
- Malformed responses or missing content are backend errors.

## Non-Goals

- No streaming response support in this slice.
- No native Ollama embeddings yet.
- No automatic local server discovery.
- No mandatory environment variables.

## Tests

- Core backend test proves request path/body and response parsing.
- CLI test proves `--ollama alias=url` registers an alias and runs a manifest using `alias:model`.
- `backends list` includes `ollama`.
