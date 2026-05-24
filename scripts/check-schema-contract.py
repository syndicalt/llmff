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
}

DOCS = [
    ROOT / "docs/schemas/README.md",
    ROOT / "docs/compatibility/core-contract-v1.md",
]


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


def main():
    require_paths([*SCHEMAS.values(), FAILURE_KINDS, *DOCS])
    for paths in FIXTURES.values():
        require_paths(paths)

    schemas = {name: load_json(path) for name, path in SCHEMAS.items()}
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
    ]
    assert failure_kinds == expected_failure_kinds
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

    validate_instance(
        schemas["run_result"],
        {
            "schema_version": 1,
            "status": "succeeded",
            "exit_code": 0,
            "manifest": {
                "hash": "sha256:"
                "0000000000000000000000000000000000000000000000000000000000000000"
            },
            "artifacts": {
                "inspect": "inspect.json",
                "trace": "trace.jsonl",
                "events": "events.jsonl",
                "checkpoint": "checkpoint.json",
            },
            "failure": None,
        },
    )
    validate_instance(
        schemas["run_result"],
        {
            "schema_version": 1,
            "status": "failed",
            "exit_code": 20,
            "manifest": {
                "hash": "sha256:"
                "1111111111111111111111111111111111111111111111111111111111111111"
            },
            "artifacts": {
                "inspect": "inspect.json",
                "trace": "trace.jsonl",
                "events": "events.jsonl",
                "checkpoint": "checkpoint.json",
            },
            "failure": {
                "kind": "stage_execution",
                "message": "tool command exited with status 7",
                "retry_recommendation": "check_stage_or_input",
            },
        },
    )

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
