#!/usr/bin/env python3
import json
from pathlib import Path

import jsonschema
import yaml


ROOT = Path(__file__).resolve().parents[1]

SCHEMAS = {
    "manifest": ROOT / "docs/schemas/pipeline-manifest-v1.schema.json",
    "event": ROOT / "docs/schemas/event-v1.schema.json",
    "trace": ROOT / "docs/schemas/trace-v1.schema.json",
    "plugin": ROOT / "docs/schemas/plugin-manifest-v1.schema.json",
    "plugin_validation_report": ROOT
    / "docs/schemas/plugin-validation-report-v1.schema.json",
    "inspect_report": ROOT / "docs/schemas/inspect-report-v1.schema.json",
    "run_result": ROOT / "docs/schemas/run-result-v1.schema.json",
}

FAILURE_KINDS = ROOT / "docs/schemas/failure-kinds-v1.json"
COMPATIBILITY_MATRIX = ROOT / "docs/compatibility/core-contract-v1-matrix.json"

SCHEMA_IDS = {
    "manifest": "pipeline-manifest-v1.schema.json",
    "event": "event-v1.schema.json",
    "trace": "trace-v1.schema.json",
    "plugin": "plugin-manifest-v1.schema.json",
    "plugin_validation_report": "plugin-validation-report-v1.schema.json",
    "inspect_report": "inspect-report-v1.schema.json",
    "run_result": "run-result-v1.schema.json",
}

FIXTURES = {
    "manifest": [
        ROOT / "fixtures/golden/manifests/minimal-pipeline.yaml",
        ROOT / "fixtures/golden/manifests/inline-compatible-pipeline.yaml",
    ],
    "event": [
        ROOT / "fixtures/golden/events/success.jsonl",
        ROOT / "fixtures/golden/events/failure.jsonl",
    ],
    "trace": [ROOT / "fixtures/golden/traces/success.jsonl"],
    "plugin": [ROOT / "fixtures/golden/plugins/valid-plugin.yaml"],
    "plugin_validation_report": [
        ROOT / "fixtures/golden/plugin-validation/valid-report.json",
        ROOT / "fixtures/golden/plugin-validation/missing-entrypoint-report.json",
    ],
    "inspect_report": [ROOT / "fixtures/golden/inspect/report.json"],
    "run_result": [
        ROOT / "fixtures/golden/run-results/success.json",
        ROOT / "fixtures/golden/run-results/stage-failure.json",
        ROOT / "fixtures/golden/run-results/interrupted.json",
    ],
    "discovery": [
        ROOT / "fixtures/golden/discovery/stages-list.json",
        ROOT / "fixtures/golden/discovery/backends-list.json",
        ROOT / "fixtures/golden/discovery/backends-report.json",
        ROOT / "fixtures/golden/discovery/models-list.json",
        ROOT / "fixtures/golden/discovery/plugins-list.json",
    ],
}

DOCS = [
    ROOT / "docs/schemas/README.md",
    ROOT / "docs/compatibility/core-contract-v1.md",
    COMPATIBILITY_MATRIX,
]

REQUIRED_COMPATIBILITY_RELEASES = ["v0.1.3", "v0.1.4", "v0.1.5"]
REQUIRED_COMPATIBILITY_SURFACES = {
    "schema",
    "event",
    "trace",
    "cli_json",
}
REQUIRED_ROADMAP_ITEMS = [1, 2, 3, 4, 5, 6, 7]


def load_json(path):
    with path.open(encoding="utf-8") as file:
        return json.load(file)


def load_yaml(path):
    with path.open(encoding="utf-8") as file:
        return yaml.safe_load(file)


def require_paths(paths):
    missing = [str(path.relative_to(ROOT)) for path in paths if not path.exists()]
    if missing:
        raise AssertionError("missing contract files: " + ", ".join(missing))


def validate_jsonl(schema, path):
    validator = jsonschema.Draft202012Validator(schema)
    with path.open(encoding="utf-8") as file:
        for line_number, line in enumerate(file, start=1):
            if not line.strip():
                continue
            instance = json.loads(line)
            validator.validate(instance)
            if instance.get("event") == "run_failed":
                assert "failure_kind" in instance
                assert "failure_message" in instance


def validate_instance(schema, instance):
    jsonschema.Draft202012Validator(schema).validate(instance)


def validate_discovery_fixture(path):
    payload = load_json(path)
    assert isinstance(payload, list), f"{path} must contain a JSON array"
    assert payload, f"{path} must contain at least one representative record"
    for item in payload:
        assert isinstance(item, dict), f"{path} records must be JSON objects"

    name = path.name
    if name == "stages-list.json":
        required = {"name", "kind", "required_fields", "optional_fields", "capabilities"}
        for item in payload:
            assert required <= set(item), f"{path} stage record missing fields"
            assert isinstance(item["required_fields"], list)
            assert isinstance(item["optional_fields"], list)
            assert isinstance(item["capabilities"], list)
    elif name == "backends-list.json":
        required = {
            "name",
            "kind",
            "registration_flag",
            "requires_api_key",
            "model_aliases",
            "capabilities",
        }
        for item in payload:
            assert required <= set(item), f"{path} backend record missing fields"
            assert isinstance(item["requires_api_key"], bool)
    elif name == "backends-report.json":
        required = {
            "name",
            "kind",
            "source",
            "requires_api_key",
            "api_key_configured",
            "capabilities",
            "diagnostics",
        }
        capability_keys = {"json_mode", "streaming", "seed", "stop", "usage_metadata"}
        for item in payload:
            assert required <= set(item), f"{path} report record missing fields"
            assert capability_keys <= set(item["capabilities"])
            for capability in capability_keys:
                detail = item["capabilities"][capability]
                assert {"supported", "detail"} <= set(detail)
                assert isinstance(detail["supported"], bool)
    elif name == "models-list.json":
        required = {
            "model",
            "backend",
            "backend_kind",
            "runtime",
            "source",
            "registration_flag",
            "requires_api_key",
            "capabilities",
        }
        for item in payload:
            assert required <= set(item), f"{path} model record missing fields"
    elif name == "plugins-list.json":
        for item in payload:
            assert {"name", "version", "capabilities"} <= set(item)
            for capability in item["capabilities"]:
                assert {"kind", "name", "entrypoint"} <= set(capability)
    else:
        raise AssertionError(f"unknown discovery fixture: {path}")


def validate_compatibility_matrix(matrix):
    assert matrix["contract"] == "core-contract-v1"
    assert matrix["compatibility_policy"] == "additive_only"
    assert matrix["release_window"] == REQUIRED_COMPATIBILITY_RELEASES
    assert matrix["roadmap_items"] == REQUIRED_ROADMAP_ITEMS
    assert set(matrix["surfaces"]) == REQUIRED_COMPATIBILITY_SURFACES
    assert len(matrix["releases"]) >= 3

    seen_releases = [release["version"] for release in matrix["releases"]]
    assert seen_releases == REQUIRED_COMPATIBILITY_RELEASES

    release_notes_dir = ROOT / "docs/release-notes"
    checked_roadmap_items = set()
    for release in matrix["releases"]:
        notes_path = release_notes_dir / f"{release['version']}.md"
        assert notes_path.exists(), f"missing release notes for {release['version']}"
        assert set(release["surfaces"]) == REQUIRED_COMPATIBILITY_SURFACES
        assert set(release["roadmap_items"]) <= set(REQUIRED_ROADMAP_ITEMS)
        checked_roadmap_items.update(release["roadmap_items"])

        for surface_name, surface in release["surfaces"].items():
            assert surface["compatibility"] in {
                "introduced",
                "preserved",
                "additive",
            }, f"invalid compatibility status for {release['version']} {surface_name}"
            assert surface["breaking_changes"] == []
            assert surface["policy"] == "additive_only"
            assert surface["notes"], f"missing notes for {release['version']} {surface_name}"

            for relative_path in surface["evidence"]:
                evidence_path = ROOT / relative_path
                assert evidence_path.exists(), f"missing evidence: {relative_path}"

    assert sorted(checked_roadmap_items) == REQUIRED_ROADMAP_ITEMS


def main():
    require_paths([*SCHEMAS.values(), FAILURE_KINDS, *DOCS])
    for paths in FIXTURES.values():
        require_paths(paths)

    schemas = {name: load_json(path) for name, path in SCHEMAS.items()}
    validate_compatibility_matrix(load_json(COMPATIBILITY_MATRIX))
    failure_kind_registry = load_json(FAILURE_KINDS)
    failure_kinds = failure_kind_registry["failure_kinds"]
    expected_failure_kinds = [
        "manifest_parse",
        "io",
        "json",
        "graph_validation",
        "unknown_stage",
        "timeout",
        "http",
        "stage_execution",
        "backend",
        "config",
        "not_implemented",
        "interrupted",
    ]
    assert failure_kinds == expected_failure_kinds
    expected_failure_modes = [
        "manifest_parse",
        "missing_input",
        "invalid_graph",
        "unknown_stage",
        "missing_backend",
        "invalid_plugin",
        "timeout",
        "http_server_error",
        "tool_nonzero",
        "schema_invalid",
        "stdout_ownership_conflict",
        "checkpoint_mismatch",
        "batch_item_failure",
        "interrupted_run",
    ]
    failure_modes = failure_kind_registry.get("failure_modes", [])
    assert [mode["mode"] for mode in failure_modes] == expected_failure_modes
    for mode in failure_modes:
        assert mode["failure_kind"] in failure_kinds
        assert mode["exit_code"] in {1, 2, 10, 20, 21, 22, 30, 130}
        assert mode["retry_recommendation"] in {
            "retry_with_backoff",
            "check_stage_or_input",
            "check_filesystem",
            "do_not_retry_without_changes",
            "resume_with_matching_checkpoint",
        }
        assert mode["stderr_contains"]
        assert isinstance(mode["run_failed_event"], bool)
        assert isinstance(mode["run_dir_result"], bool)
    for name, schema in schemas.items():
        jsonschema.Draft202012Validator.check_schema(schema)
        assert schema["$id"].endswith(SCHEMA_IDS[name])

    for path in FIXTURES["manifest"]:
        manifest = load_yaml(path)
        validate_instance(schemas["manifest"], manifest)
        assert manifest["version"] == 1
        assert manifest["metadata"]["inline_graph_syntax_version"] == 1

    for path in FIXTURES["plugin"]:
        validate_instance(schemas["plugin"], load_yaml(path))

    for path in FIXTURES["plugin_validation_report"]:
        validate_instance(schemas["plugin_validation_report"], load_json(path))

    for path in FIXTURES["inspect_report"]:
        report = load_json(path)
        validate_instance(schemas["inspect_report"], report)
        assert report["format_version"] == 1

    for path in FIXTURES["run_result"]:
        result = load_json(path)
        validate_instance(schemas["run_result"], result)
        assert result["schema_version"] == 1

    for path in FIXTURES["discovery"]:
        validate_discovery_fixture(path)

    for path in FIXTURES["event"]:
        validate_jsonl(schemas["event"], path)

    for path in FIXTURES["trace"]:
        validate_jsonl(schemas["trace"], path)

    for schema_name in ["event", "trace"]:
        assert schemas[schema_name]["properties"]["failure_kind"]["enum"] == failure_kinds

    event_schema_text = (ROOT / "examples/supervision/fixtures/event.schema.json").read_text(
        encoding="utf-8"
    )
    assert '"enum"' in event_schema_text
    for failure_kind in failure_kinds:
        for path in [ROOT / "docs/events.md", ROOT / "docs/compatibility/core-contract-v1.md"]:
            assert failure_kind in path.read_text(encoding="utf-8")


if __name__ == "__main__":
    main()
