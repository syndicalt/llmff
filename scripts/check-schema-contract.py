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
}

SCHEMA_IDS = {
    "manifest": "pipeline-manifest-v1.schema.json",
    "event": "event-v1.schema.json",
    "trace": "trace-v1.schema.json",
    "plugin": "plugin-manifest-v1.schema.json",
    "plugin_validation_report": "plugin-validation-report-v1.schema.json",
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
    require_paths([*SCHEMAS.values(), *DOCS])
    for paths in FIXTURES.values():
        require_paths(paths)

    schemas = {name: load_json(path) for name, path in SCHEMAS.items()}
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

    for path in FIXTURES["event"]:
        validate_jsonl(schemas["event"], path)

    for path in FIXTURES["trace"]:
        validate_jsonl(schemas["trace"], path)


if __name__ == "__main__":
    main()
