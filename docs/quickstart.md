# Quickstart

This guide gets a new `llmff` user from install to a working pipeline without
requiring provider credentials. It uses the deterministic mock backend first,
then shows the smallest real-backend configuration.

## 1. Install

Install the latest tagged release with Cargo:

```bash
cargo install --git https://github.com/syndicalt/llmff --tag v0.1.3 llmff
```

If you downloaded a release archive instead, unpack it and put the `llmff`
binary on your `PATH`. Platform-specific release asset installation and
checksum verification are documented in
[`docs/github-release-installation.md`](github-release-installation.md).

Verify the binary:

```bash
llmff --version
llmff stages list
```

Expected version:

```text
llmff 0.1.3
```

## 2. Run The Offline Example

Clone the repository, then run the JSON repair example:

```bash
git clone https://github.com/syndicalt/llmff.git
cd llmff

LLMFF_MOCK_BAD_RESPONSE='{"wrong":true}' \
LLMFF_MOCK_GOOD_RESPONSE='{"answer":"ok"}' \
llmff run examples/json-repair.yaml --trace /tmp/llmff-trace.jsonl
```

The run writes:

```text
examples/answer.json
```

Confirm the output:

```bash
cat examples/answer.json
```

Expected output:

```json
{"answer":"ok"}
```

The example intentionally asks one mock model for invalid JSON, validates it
against `examples/answer.schema.json`, repairs it with a second mock model, and
writes the repaired object.

## 3. Inspect Before Running

Use `inspect` when editing a pipeline. It validates references, stage
requirements, type compatibility that can be proven statically, and backend
availability without calling model servers.

```bash
llmff inspect examples/json-repair.yaml
```

Expected output:

```text
ok
```

## 4. Run A One-Line Pipeline

Inline graphs are useful for shell workflows and quick experiments:

```bash
LLMFF_MOCK_GOOD_RESPONSE='{"answer":"ok"}' \
llmff run -i examples/question.txt \
  -g 'load | infer(model=mock:good) | write(-)'
```

Expected output:

```json
{"answer":"ok"}
```

## 5. Use A Real Backend

Register an OpenAI-compatible backend explicitly:

```bash
export OPENAI_API_KEY='...'

llmff run examples/json-repair.yaml \
  --backend openai=https://api.openai.com/v1 \
  --api-key-env openai=OPENAI_API_KEY
```

Manifests reference that backend with model ids such as:

```yaml
model: openai:gpt-4.1-mini
```

For a local Ollama server:

```bash
llmff run pipeline.yaml \
  --ollama ollama=http://localhost:11434
```

Then use model ids such as:

```yaml
model: ollama:llama3.1
```

## 6. Read The Trace

The `--trace` file is JSONL. Summarize it with:

```bash
llmff trace /tmp/llmff-trace.jsonl
```

Trace output reports run status, stage status, durations, safe model metadata,
validation errors, cache hits, and token usage when a backend reports it. It
does not include full prompts, tool bodies, headers, cached values, or secrets.

## Troubleshooting

- `llmff: command not found`: confirm Cargo's bin directory is on `PATH`.
  Cargo usually installs to `$HOME/.cargo/bin`.
- `mock backend does not serve model`: set the matching mock response variable
  for the model id in the manifest. `mock:good` uses
  `LLMFF_MOCK_GOOD_RESPONSE`; `mock:bad` uses `LLMFF_MOCK_BAD_RESPONSE`.
- `backend alias is not registered`: pass `--backend alias=url` or
  `--ollama alias=url`, and make sure the manifest model id uses the same alias
  before the first colon.
- `missing API key environment variable`: export the secret and pass
  `--api-key-env alias=ENV_NAME`.
- Windows and macOS release installers are unsigned in `v0.1.3`; expect normal
  OS trust prompts until paid signing and notarization are added.

## Next Steps

- See [`examples/README.md`](../examples/README.md) for the example catalog.
- See [`docs/provider-troubleshooting.md`](provider-troubleshooting.md) for
  OpenAI-compatible and Ollama setup notes.
- See [`README.md`](../README.md) for the CLI reference and stage semantics.
- See [`docs/platform-support.md`](platform-support.md) for release artifacts
  and installer assumptions.
- See [`docs/github-release-installation.md`](github-release-installation.md)
  for direct GitHub Release installation.
