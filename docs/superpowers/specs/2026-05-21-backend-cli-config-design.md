# Backend CLI Configuration Design

## Purpose

Add real OpenAI-compatible backend execution to `llmff run` while preserving the FFmpeg-like rule that the command line is the primary recipe.

Environment variables may supply secrets or optional defaults, but they must not be mandatory for normal backend registration.

## User Experience

Backend registration is explicit on the run command:

```bash
llmff run pipeline.yaml \
  --backend openai=https://api.openai.com/v1 \
  --api-key-env openai=OPENAI_API_KEY
```

A manifest stage references a model through the registered backend alias:

```yaml
graph:
  - id: draft
    op: infer
    from: load_prompt
    model: openai:gpt-4.1-mini
```

The backend alias is the string before the first colon in the model id. The backend model is the string after the first colon.

Examples:

- `openai:gpt-4.1-mini` uses backend alias `openai` and provider model `gpt-4.1-mini`.
- `local:Qwen2.5-7B-Instruct` uses backend alias `local` and provider model `Qwen2.5-7B-Instruct`.
- Existing mock ids such as `mock:good` continue to work as exact mock model ids.

## CLI Flags

`llmff run` accepts repeatable backend flags:

```text
--backend <alias>=<base_url>
--api-key-env <alias>=<env_var_name>
--api-key <alias>=<literal_key>
```

Rules:

- `--backend` is the primary declaration. It is required for non-mock aliases.
- `--api-key-env` reads the named environment variable for the matching alias.
- `--api-key` is allowed for local/testing servers but should not be shown in docs as the preferred path.
- If both `--api-key-env` and `--api-key` are present for the same alias, the explicit literal `--api-key` wins.
- Missing API key is allowed because many local OpenAI-compatible servers do not require one.
- No API key value is written to traces, stdout, stderr, or README examples.

Optional convenience environment variables may be added later, but this slice does not add hidden env-derived backend registration.

## Backend Resolution

The engine currently registers backends by exact model id. This slice changes backend lookup to support exact and alias-based resolution:

1. Try exact key match, preserving mock behavior.
2. If no exact match exists, split model id at the first colon.
3. Find a backend registered under the alias.
4. Send the provider model id, not the full `alias:model`, to the backend.
5. If no backend is found, return a clear error naming the unresolved alias/model.

## Observability And Secrets

Trace events remain lifecycle-oriented for this slice. They do not include prompts, API keys, or request bodies.

Errors may include backend alias, model id, HTTP status, and response body. Errors must never include the configured API key.

## Testing

Use deterministic tests only:

- Unit tests for backend model id resolution.
- CLI integration test using `wiremock` to prove `llmff run --backend openai=<server>` posts to `/v1/chat/completions`.
- CLI test proving `--api-key-env openai=TEST_KEY_ENV` sets Bearer auth without exposing the key in stdout or stderr.
- CLI test proving missing backend alias fails clearly.

No test should call a real external API.
