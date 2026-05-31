#!/bin/sh
set -eu

plugin_dir="examples/plugins"

while [ "$#" -gt 0 ]; do
  case "$1" in
    --plugin-dir)
      if [ "$#" -lt 2 ]; then
        echo "missing value for --plugin-dir" >&2
        exit 2
      fi
      plugin_dir="$2"
      shift 2
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
registry="$root/docs/plugins/registry.v1.json"
fixtures="$root/docs/plugins/fixtures/protocol-v1"

case "$plugin_dir" in
  /*) resolved_plugin_dir="$plugin_dir" ;;
  *) resolved_plugin_dir="$root/$plugin_dir" ;;
esac

if [ "${LLMFF_BIN:-}" ]; then
  set -- "$LLMFF_BIN"
else
  set -- cargo run -q -p llmff --
fi

"$@" plugins validate --plugin-dir "$resolved_plugin_dir" >/dev/null

python3 - "$registry" "$fixtures" "$root" <<'PY'
import json
import pathlib
import re
import sys

registry_path = pathlib.Path(sys.argv[1])
fixtures = pathlib.Path(sys.argv[2])
root = pathlib.Path(sys.argv[3])

registry = json.loads(registry_path.read_text())
if registry.get("format_version") != 1:
    raise SystemExit("registry format_version must be 1")
if registry.get("plugin_protocol_version") != 1:
    raise SystemExit("registry plugin_protocol_version must be 1")

required = {
    "retrieval-provider",
    "reranker",
    "model-backend",
    "sampler",
    "tool-transport",
    "postprocessor",
}
categories = {plugin.get("category") for plugin in registry.get("plugins", [])}
missing = sorted(required - categories)
if missing:
    raise SystemExit(f"registry missing categories: {', '.join(missing)}")

template_manifest = root / "examples/plugins/template/llmff-plugin.yaml"
template_readme = root / "examples/plugins/template/README.md"
if not template_manifest.is_file():
    raise SystemExit(f"missing plugin template manifest: {template_manifest}")
if not template_readme.is_file():
    raise SystemExit(f"missing plugin template README: {template_readme}")
template_manifest_text = template_manifest.read_text()
for text in [
    "kind: stage",
    "kind: backend",
    "kind: sampler",
    "kind: tool-transport",
    "name: template.uppercase",
    "name: template-echo",
    "name: template-deterministic",
    "name: template-stdio",
]:
    if text not in template_manifest_text:
        raise SystemExit(f"plugin template manifest must contain: {text}")
template_readme_text = template_readme.read_text()
for text in [
    "protocol version 1",
    "unsandboxed local executables",
    "docs/plugins/fixtures/protocol-v1",
]:
    if text not in template_readme_text:
        raise SystemExit(f"plugin template README must contain: {text}")

if registry.get("promotion_policy") != "promotion-policy.md":
    raise SystemExit("registry must link promotion_policy to promotion-policy.md")

policy = registry_path.parent / registry["promotion_policy"]
if not policy.is_file():
    raise SystemExit(f"registry promotion policy is missing: {policy}")
policy_text = policy.read_text()
for text in [
    "Promotion is a support commitment",
    "Promoted registry entries",
    "review evidence",
    "trust-boundary review",
]:
    if text not in policy_text:
        raise SystemExit(f"promotion policy missing required text: {text}")

required_plugin_fields = {
    "name",
    "version",
    "category",
    "manifest",
    "capabilities",
    "summary",
    "promotion",
    "trust_boundary",
}
required_review_fields = {
    "format_version",
    "plugin",
    "version",
    "category",
    "manifest",
    "promotion",
    "validation",
    "trust_boundary",
    "decision",
}
required_trust_fields = {
    "sandboxed",
    "filesystem_access",
    "network_access",
    "process_access",
    "environment_access",
}
expected_contracts_by_category = {
    "retrieval-provider": {"stage/stdin.txt", "stage/stdout.txt"},
    "reranker": {"stage/stdin.txt", "stage/stdout.txt"},
    "model-backend": {"backend/infer-request.json", "backend/infer-response.json"},
    "sampler": {"sampler/infer-request.json", "sampler/overrides.json"},
    "tool-transport": {"tool-transport/stdin.txt", "tool-transport/stdout.txt"},
    "postprocessor": {"stage/stdin.txt", "stage/stdout.txt"},
}
review_paths = set()

for plugin in registry.get("plugins", []):
    missing_fields = sorted(required_plugin_fields - set(plugin))
    if missing_fields:
        raise SystemExit(
            f"registry plugin {plugin.get('name', '<unknown>')} missing fields: "
            + ", ".join(missing_fields)
        )

    promotion = plugin["promotion"]
    if promotion.get("status") != "promoted":
        raise SystemExit(f"registry plugin {plugin['name']} must be promoted")
    if promotion.get("support_commitment") != "protocol-v1-fixture-backed":
        raise SystemExit(
            f"registry plugin {plugin['name']} missing protocol-v1 support commitment"
        )
    if promotion.get("policy") != registry["promotion_policy"]:
        raise SystemExit(f"registry plugin {plugin['name']} promotion policy mismatch")
    reviewed_at = promotion.get("reviewed_at")
    if not isinstance(reviewed_at, str) or not re.fullmatch(r"\d{4}-\d{2}-\d{2}", reviewed_at):
        raise SystemExit(f"registry plugin {plugin['name']} missing review date")
    review_path = promotion.get("review")
    if (
        not isinstance(review_path, str)
        or not review_path.startswith("reviews/")
        or not review_path.endswith(".json")
        or ".." in pathlib.PurePosixPath(review_path).parts
    ):
        raise SystemExit(f"registry plugin {plugin['name']} has invalid review path")
    if review_path in review_paths:
        raise SystemExit(f"duplicate plugin review path: {review_path}")
    review_paths.add(review_path)

    trust_boundary = plugin["trust_boundary"]
    missing_trust = sorted(required_trust_fields - set(trust_boundary))
    if missing_trust:
        raise SystemExit(
            f"registry plugin {plugin['name']} missing trust fields: "
            + ", ".join(missing_trust)
        )
    if trust_boundary.get("sandboxed") is not False:
        raise SystemExit(f"registry plugin {plugin['name']} must declare unsandboxed execution")

    manifest_path = (registry_path.parent / plugin["manifest"]).resolve()
    if not manifest_path.is_file() or root not in manifest_path.parents:
        raise SystemExit(f"registry plugin {plugin['name']} manifest path is invalid")
    manifest_text = manifest_path.read_text()
    for text in [f"name: {plugin['name']}", f"version: {plugin['version']}"]:
        if text not in manifest_text:
            raise SystemExit(f"registry plugin {plugin['name']} does not match manifest")

    review = registry_path.parent / review_path
    if not review.is_file():
        raise SystemExit(f"registry plugin {plugin['name']} missing review: {review_path}")
    review_json = json.loads(review.read_text())
    missing_review = sorted(required_review_fields - set(review_json))
    if missing_review:
        raise SystemExit(
            f"plugin review {review_path} missing fields: " + ", ".join(missing_review)
        )
    for key in ["plugin", "version", "category", "manifest"]:
        expected = plugin["name"] if key == "plugin" else plugin[key]
        if review_json.get(key) != expected:
            raise SystemExit(f"plugin review {review_path} {key} mismatch")
    if review_json["promotion"] != promotion:
        raise SystemExit(f"plugin review {review_path} promotion mismatch")
    if review_json["trust_boundary"] != trust_boundary:
        raise SystemExit(f"plugin review {review_path} trust boundary mismatch")
    if review_json.get("format_version") != 1:
        raise SystemExit(f"plugin review {review_path} format_version must be 1")
    if review_json.get("decision") != "approved-for-registry-promotion":
        raise SystemExit(f"plugin review {review_path} must approve promotion")

    validation = review_json["validation"]
    if validation.get("command") != "scripts/check-plugin-fixtures.sh":
        raise SystemExit(f"plugin review {review_path} missing validation command")
    if validation.get("manifest_checked") is not True:
        raise SystemExit(f"plugin review {review_path} must check manifest")
    contracts = set(validation.get("protocol_fixtures", []))
    expected_contracts = expected_contracts_by_category[plugin["category"]]
    if contracts != expected_contracts:
        raise SystemExit(
            f"plugin review {review_path} protocol fixture mismatch: "
            + ", ".join(sorted(contracts))
        )

for path in fixtures.rglob("*.json"):
    json.loads(path.read_text())

readme = fixtures / "README.md"
readme_text = readme.read_text()
for text in [
    "backend/infer-request.json",
    "backend/infer-response.json",
    "sampler/infer-request.json",
    "sampler/overrides.json",
    "stage/stdin.txt",
    "stage/stdout.txt",
    "tool-transport/stdin.txt",
    "tool-transport/stdout.txt",
]:
    if text not in readme_text:
        raise SystemExit(f"protocol fixture README must document {text}")

backend_request = json.loads((fixtures / "backend/infer-request.json").read_text())
backend_response = json.loads((fixtures / "backend/infer-response.json").read_text())
sampler_request = json.loads((fixtures / "sampler/infer-request.json").read_text())
sampler_overrides = json.loads((fixtures / "sampler/overrides.json").read_text())

for name, payload in [
    ("backend request", backend_request),
    ("sampler request", sampler_request),
]:
    for key in [
        "model",
        "messages",
        "temperature",
        "top_p",
        "max_tokens",
        "seed",
        "response_format",
        "stop",
    ]:
        if key not in payload:
            raise SystemExit(f"{name} missing {key}")
    if not payload["messages"]:
        raise SystemExit(f"{name} must include at least one message")

if not isinstance(backend_response.get("text"), str) or not backend_response["text"]:
    raise SystemExit("backend response must include non-empty text")
usage = backend_response.get("usage")
if not isinstance(usage, dict):
    raise SystemExit("backend response must include usage object")
for key in ["prompt_tokens", "completion_tokens", "total_tokens"]:
    if not isinstance(usage.get(key), int):
        raise SystemExit(f"backend response usage missing integer {key}")

allowed_sampler_keys = {
    "temperature",
    "top_p",
    "max_tokens",
    "seed",
    "response_format",
    "stop",
}
unknown_sampler_keys = sorted(set(sampler_overrides) - allowed_sampler_keys)
if unknown_sampler_keys:
    raise SystemExit(
        "sampler overrides contain unsupported keys: "
        + ", ".join(unknown_sampler_keys)
    )
if not sampler_overrides:
    raise SystemExit("sampler overrides fixture must include at least one override")

for subdir in ["stage", "tool-transport"]:
    stdin = (fixtures / subdir / "stdin.txt").read_text()
    stdout = (fixtures / subdir / "stdout.txt").read_text()
    if not stdin.strip() or not stdout.strip():
        raise SystemExit(f"{subdir} stdin/stdout fixtures must be non-empty")

print("plugin fixtures ok")
PY
