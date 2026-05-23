# Provider Smoke Readiness

Live provider smoke jobs certify that a provider path works against a real
endpoint at a point in time. Provider certification is a support commitment: maintainers
must be ready to own failed jobs, provider drift, runner setup, secret rotation,
and user reports before treating a live smoke as a supported channel.

## Opt-In Gate

Live provider smokes must remain opt-in:

- The GitHub workflow is started with `workflow_dispatch`.
- The workflow must not run on pull_request or push.
- Smoke scripts skip unless `LLMFF_LIVE_PROVIDER_SMOKE=1` is set.
- Local scripts must exit successfully without secrets when the opt-in flag is
  absent, so contributors can run repository checks offline.

## OpenAI-Compatible Smoke

Required configuration:

- `OPENAI_API_KEY`: GitHub Actions secret for the provider key.
- `OPENAI_BASE_URL`: optional GitHub Actions variable or shell variable. It
  defaults to `https://api.openai.com/v1`.
- `LLMFF_LIVE_PROVIDER_SMOKE=1`: explicit opt-in flag.
- Runner: `ubuntu-latest` with a Rust toolchain and outbound HTTPS access.

The smoke script builds `llmff`, runs
`examples/providers/openai-compatible.yaml`, and checks that the answer artifact
was written. It does not print the API key.

## Ollama Smoke

Required configuration:

- `OLLAMA_BASE_URL`: endpoint for a reachable Ollama service. The GitHub
  workflow uses `http://localhost:11434`.
- `LLMFF_LIVE_PROVIDER_SMOKE=1`: explicit opt-in flag.
- Runner: `ubuntu-latest` with permission to install Ollama, start
  `ollama serve`, pull the requested model, and use enough disk and memory for
  the model.

The workflow must keep Ollama model setup explicit because model size, download
time, and runner capacity are support assumptions.

## Maintainer Checklist

Before running or advertising provider certification:

- Confirm the provider choice, model, and expected request fields are still
  supported.
- Confirm secrets and variables are configured for the repository.
- Confirm runner setup and network access are acceptable for the provider.
- Run the smoke manually from `workflow_dispatch` and inspect failures before
  announcing support.
- Keep failures actionable: missing secret means skipped or failed setup,
  provider HTTP failures mean provider compatibility work, and runner failures
  mean workflow support work.
