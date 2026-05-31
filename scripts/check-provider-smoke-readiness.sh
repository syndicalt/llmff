#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$repo_root"

require_file() {
  local path="$1"
  if [ ! -f "$path" ]; then
    printf 'error: missing provider smoke readiness artifact: %s\n' "$path" >&2
    exit 1
  fi
}

require_text() {
  local path="$1"
  local text="$2"
  require_file "$path"
  if ! grep -Fq -- "$text" "$path"; then
    printf 'error: %s must contain: %s\n' "$path" "$text" >&2
    exit 1
  fi
}

require_absent_text() {
  local path="$1"
  local text="$2"
  require_file "$path"
  if grep -Fq -- "$text" "$path"; then
    printf 'error: %s must not contain: %s\n' "$path" "$text" >&2
    exit 1
  fi
}

guide="docs/provider-smoke-readiness.md"
workflow=".github/workflows/live-provider-smoke.yml"
openai_script="scripts/smoke-openai-compatible-provider.sh"
ollama_script="scripts/smoke-ollama-provider.sh"
capability_report_script="scripts/provider-capability-report.sh"
support_tiers="docs/providers/support-tiers.md"
live_smoke_history="docs/providers/live-smoke-history.json"

require_file "$guide"
require_file "$workflow"
require_file "$openai_script"
require_file "$ollama_script"
require_file "$capability_report_script"
require_file "$support_tiers"
require_file "$live_smoke_history"

for text in \
  "LLMFF_LIVE_PROVIDER_SMOKE=1" \
  "OPENAI_API_KEY" \
  "OPENAI_BASE_URL" \
  "OLLAMA_BASE_URL" \
  "ubuntu-latest" \
  "workflow_dispatch" \
  "not run on pull_request or push" \
  "provider capability report" \
  "certification is a support commitment"
do
  require_text "$guide" "$text"
done

require_text "$workflow" "workflow_dispatch"
require_absent_text "$workflow" "pull_request"
require_absent_text "$workflow" "push:"
require_text "$workflow" "runs-on: ubuntu-latest"
require_text "$workflow" "LLMFF_LIVE_PROVIDER_SMOKE: \"1\""
require_text "$workflow" "secrets.OPENAI_API_KEY"
require_text "$workflow" "vars.OPENAI_BASE_URL"
require_text "$workflow" "OLLAMA_BASE_URL: http://localhost:11434"

require_text "$openai_script" "LLMFF_LIVE_PROVIDER_SMOKE"
require_text "$openai_script" "OPENAI_API_KEY"
require_text "$openai_script" "OPENAI_BASE_URL"
require_text "$openai_script" "exit 0"
require_text "$ollama_script" "LLMFF_LIVE_PROVIDER_SMOKE"
require_text "$ollama_script" "OLLAMA_BASE_URL"
require_text "$ollama_script" "exit 0"

python3 <<'PY'
from pathlib import Path
import json

providers = {
    "anthropic": {
        "api_key_env": "ANTHROPIC_ADAPTER_API_KEY",
        "tier": "mock-inspectable",
        "live_smoke": "not configured",
        "quirk": "adapter",
    },
    "azure-openai": {
        "api_key_env": "AZURE_OPENAI_API_KEY",
        "tier": "mock-inspectable",
        "live_smoke": "not configured",
        "quirk": "deployment",
    },
    "groq": {
        "api_key_env": "GROQ_API_KEY",
        "tier": "mock-inspectable",
        "live_smoke": "not configured",
        "quirk": "model-level",
    },
    "lm-studio": {
        "api_key_env": None,
        "tier": "mock-inspectable",
        "live_smoke": "not configured",
        "quirk": "loaded local model",
    },
    "localai": {
        "api_key_env": None,
        "tier": "mock-inspectable",
        "live_smoke": "not configured",
        "quirk": "model backends differ",
    },
    "ollama": {
        "api_key_env": None,
        "tier": "opt-in smoke ready",
        "live_smoke": "workflow_dispatch",
        "quirk": "non-streaming",
    },
    "openai": {
        "api_key_env": "OPENAI_API_KEY",
        "tier": "mock-inspectable",
        "live_smoke": "via openai-compatible",
        "quirk": "response_format",
    },
    "openai-compatible": {
        "api_key_env": "OPENAI_API_KEY",
        "tier": "opt-in smoke ready",
        "live_smoke": "workflow_dispatch",
        "quirk": "base URL normalization",
    },
    "openrouter": {
        "api_key_env": "OPENROUTER_API_KEY",
        "tier": "mock-inspectable",
        "live_smoke": "not configured",
        "quirk": "routed upstream model",
    },
    "together": {
        "api_key_env": "TOGETHER_API_KEY",
        "tier": "mock-inspectable",
        "live_smoke": "not configured",
        "quirk": "strict JSON",
    },
    "vllm": {
        "api_key_env": None,
        "tier": "mock-inspectable",
        "live_smoke": "not configured",
        "quirk": "served model",
    },
}

support_tiers = Path("docs/providers/support-tiers.md")
support_tiers_text = support_tiers.read_text(encoding="utf-8")
for required in [
    "## Tier Definitions",
    "## Provider Matrix",
    "documented only",
    "mock-inspectable",
    "opt-in smoke ready",
    "live-smoke verified",
    "no provider is live-smoke verified",
]:
    if required not in support_tiers_text:
        raise SystemExit(f"{support_tiers} must contain: {required}")

history_path = Path("docs/providers/live-smoke-history.json")
try:
    history = json.loads(history_path.read_text(encoding="utf-8"))
except json.JSONDecodeError as exc:
    raise SystemExit(f"{history_path} must be valid JSON: {exc}") from exc

if history.get("schema_version") != 1:
    raise SystemExit(f"{history_path} must set schema_version to 1")
history_providers = {entry.get("provider"): entry for entry in history.get("providers", [])}

for provider, expected in providers.items():
    doc = Path(f"docs/providers/{provider}.md")
    example = Path(f"examples/providers/{provider}.yaml")
    if not doc.exists():
        raise SystemExit(f"missing provider doc: {doc}")
    if not example.exists():
        raise SystemExit(f"missing provider example manifest: {example}")
    doc_text = doc.read_text(encoding="utf-8")
    normalized_doc_text = " ".join(doc_text.split())
    example_text = example.read_text(encoding="utf-8")
    for required in [
        "## Support Tier",
        f"Support tier: {expected['tier']}",
        "## Capabilities",
        "## Quirks",
        "## Live Smoke",
        f"Live smoke: {expected['live_smoke']}",
        "llmff backends report",
        f"llmff run examples/providers/{provider}.yaml",
        "Compatibility:",
        "JSON mode",
        "streaming",
        "seed",
        "stop",
        "usage metadata",
        expected["quirk"],
    ]:
        if required not in doc_text and required not in normalized_doc_text:
            raise SystemExit(f"{doc} must contain: {required}")
    if expected["api_key_env"] and expected["api_key_env"] not in doc_text:
        raise SystemExit(f"{doc} must document {expected['api_key_env']}")
    if f"| `{provider}` | {expected['tier']} |" not in support_tiers_text:
        raise SystemExit(f"{support_tiers} must include provider row for {provider}")
    history_entry = history_providers.get(provider)
    if not history_entry:
        raise SystemExit(f"{history_path} must include provider entry for {provider}")
    if history_entry.get("support_tier") != expected["tier"]:
        raise SystemExit(f"{history_path} support_tier mismatch for {provider}")
    if history_entry.get("live_smoke") != expected["live_smoke"]:
        raise SystemExit(f"{history_path} live_smoke mismatch for {provider}")
    if "readiness_evidence" not in history_entry:
        raise SystemExit(f"{history_path} must include readiness_evidence for {provider}")
    for required in ["version: 1", "op: infer", "response_format: json", "op: validate_json"]:
        if required not in example_text:
            raise SystemExit(f"{example} must contain: {required}")
PY

printf 'provider smoke readiness validation succeeded\n'
