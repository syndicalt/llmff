use std::path::Path;

use serde_json::{json, Value as JsonValue};

use crate::error::LlmffError;
use crate::manifest::StageSpec;
use crate::value::{StageStatus, Value};

use super::json_path::get_json_path;
use super::{parse_json_stage_input, render_messages_as_text};

pub(super) fn predicate(
    spec: &StageSpec,
    input: Option<Value>,
    _cwd: &Path,
) -> Result<StageStatus, LlmffError> {
    let value = input.ok_or_else(|| LlmffError::StageExecution {
        stage_id: spec.id.clone(),
        message: "predicate requires input".to_string(),
    })?;
    let json_input = match value {
        Value::Json(json) => json,
        Value::Text(text) => parse_json_stage_input(spec, &text)?,
        Value::Messages(messages) => {
            parse_json_stage_input(spec, &render_messages_as_text(&messages))?
        }
    };
    let path = spec.json_path.as_deref().or(spec.field.as_deref());
    let observed = match path {
        Some(path) => get_json_path(&json_input, path),
        None => Some(&json_input),
    };
    let mode = spec.mode.as_deref().unwrap_or("truthy");
    let requires_value = matches!(mode, "equals" | "gt" | "gte" | "lt" | "lte");
    if requires_value && spec.value.is_none() {
        return Err(LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: format!("predicate mode `{mode}` requires value"),
        });
    }

    let passed = match mode {
        "truthy" => observed.map(is_truthy).unwrap_or(false),
        "exists" => observed.is_some(),
        "equals" => observed == spec.value.as_ref(),
        "gt" => compare_numbers(observed, spec.value.as_ref(), |actual, expected| {
            actual > expected
        }),
        "gte" => compare_numbers(observed, spec.value.as_ref(), |actual, expected| {
            actual >= expected
        }),
        "lt" => compare_numbers(observed, spec.value.as_ref(), |actual, expected| {
            actual < expected
        }),
        "lte" => compare_numbers(observed, spec.value.as_ref(), |actual, expected| {
            actual <= expected
        }),
        "contains" => contains_value(observed, spec.value.as_ref()),
        _ => {
            return Err(LlmffError::StageExecution {
                stage_id: spec.id.clone(),
                message: format!(
                    "predicate mode must be truthy, exists, equals, gt, gte, lt, lte, or contains; got `{mode}`"
                ),
            });
        }
    };

    let mut payload = json!({
        "passed": passed,
        "mode": mode,
        "observed": observed.cloned().unwrap_or(JsonValue::Null)
    });
    payload["path"] = JsonValue::String(path.unwrap_or("").to_string());
    if let Some(expected) = &spec.value {
        payload["expected"] = expected.clone();
    }

    Ok(StageStatus::Success(Value::Json(payload)))
}

fn is_truthy(value: &JsonValue) -> bool {
    match value {
        JsonValue::Bool(value) => *value,
        JsonValue::Null => false,
        JsonValue::Number(number) => number.as_f64().is_some_and(|value| value != 0.0),
        JsonValue::String(value) => !value.is_empty(),
        JsonValue::Array(value) => !value.is_empty(),
        JsonValue::Object(value) => !value.is_empty(),
    }
}

fn compare_numbers(
    observed: Option<&JsonValue>,
    expected: Option<&JsonValue>,
    predicate: impl FnOnce(f64, f64) -> bool,
) -> bool {
    let Some(actual) = observed.and_then(JsonValue::as_f64) else {
        return false;
    };
    let Some(expected) = expected.and_then(JsonValue::as_f64) else {
        return false;
    };

    actual.is_finite() && expected.is_finite() && predicate(actual, expected)
}

fn contains_value(observed: Option<&JsonValue>, expected: Option<&JsonValue>) -> bool {
    match (observed, expected) {
        (Some(JsonValue::String(actual)), Some(JsonValue::String(expected))) => {
            actual.contains(expected)
        }
        (Some(JsonValue::Array(items)), Some(expected)) => {
            items.iter().any(|item| item == expected)
        }
        (Some(JsonValue::Object(object)), Some(JsonValue::String(expected))) => {
            object.contains_key(expected)
        }
        (Some(JsonValue::String(actual)), None) => !actual.is_empty(),
        (Some(JsonValue::Array(items)), None) => !items.is_empty(),
        (Some(JsonValue::Object(object)), None) => !object.is_empty(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    use serde_json::json;

    use crate::manifest::Manifest;
    use crate::value::{StageStatus, Value};

    fn stage_from_yaml(yaml: &str) -> crate::manifest::StageSpec {
        let mut manifest = Manifest::from_yaml_str(yaml).expect("manifest should parse");
        manifest.graph.remove(0)
    }

    fn passed(status: StageStatus) -> bool {
        match status {
            StageStatus::Success(Value::Json(value)) => value["passed"].as_bool().unwrap(),
            other => panic!("unexpected status: {other:?}"),
        }
    }

    #[test]
    fn truthy_mode_passes_for_true_field() {
        let spec = stage_from_yaml(
            r#"
version: 1
graph:
  - id: ready
    op: predicate
    from: check
    field: ok
    mode: truthy
"#,
        );

        let status = predicate(
            &spec,
            Some(Value::Json(json!({"ok": true}))),
            Path::new("."),
        )
        .unwrap();

        assert!(passed(status));
    }

    #[test]
    fn exists_mode_passes_for_present_null_field() {
        let spec = stage_from_yaml(
            r#"
version: 1
graph:
  - id: ready
    op: predicate
    from: check
    field: maybe
    mode: exists
"#,
        );

        let status = predicate(
            &spec,
            Some(Value::Json(json!({"maybe": null}))),
            Path::new("."),
        )
        .unwrap();

        assert!(passed(status));
    }

    #[test]
    fn numeric_comparison_uses_expected_value() {
        let spec = stage_from_yaml(
            r#"
version: 1
graph:
  - id: ready
    op: predicate
    from: check
    json_path: result.score
    mode: gte
    value: 7
"#,
        );

        let status = predicate(
            &spec,
            Some(Value::Json(json!({"result": {"score": 8}}))),
            Path::new("."),
        )
        .unwrap();

        assert!(passed(status));
    }

    #[test]
    fn contains_mode_checks_strings_and_arrays() {
        let spec = stage_from_yaml(
            r#"
version: 1
graph:
  - id: ready
    op: predicate
    from: check
    field: tags
    mode: contains
    value: safe
"#,
        );

        let status = predicate(
            &spec,
            Some(Value::Json(json!({"tags": ["safe", "cited"]}))),
            Path::new("."),
        )
        .unwrap();

        assert!(passed(status));
    }

    #[test]
    fn contains_mode_can_check_non_empty_collection_without_value() {
        let spec = stage_from_yaml(
            r#"
version: 1
graph:
  - id: ready
    op: predicate
    from: check
    field: tags
    mode: contains
"#,
        );

        let status = predicate(
            &spec,
            Some(Value::Json(json!({"tags": ["safe", "cited"]}))),
            Path::new("."),
        )
        .unwrap();

        assert!(passed(status));
    }

    #[test]
    fn reports_empty_path_when_evaluating_whole_input() {
        let spec = stage_from_yaml(
            r#"
version: 1
graph:
  - id: ready
    op: predicate
    from: check
    mode: truthy
"#,
        );

        let status = predicate(
            &spec,
            Some(Value::Json(json!({"ok": true}))),
            Path::new("."),
        )
        .unwrap();
        let StageStatus::Success(Value::Json(report)) = status else {
            panic!("unexpected status");
        };

        assert_eq!(report["path"], "");
        assert_eq!(report["observed"], json!({"ok": true}));
    }

    #[test]
    fn equals_mode_reports_false_for_mismatch() {
        let spec = stage_from_yaml(
            r#"
version: 1
graph:
  - id: ready
    op: predicate
    from: check
    field: label
    mode: equals
    value: pass
"#,
        );

        let status = predicate(
            &spec,
            Some(Value::Json(json!({"label": "fail"}))),
            Path::new("."),
        )
        .unwrap();

        assert!(!passed(status));
    }

    #[test]
    fn rejects_comparison_without_value() {
        let spec = stage_from_yaml(
            r#"
version: 1
graph:
  - id: ready
    op: predicate
    from: check
    field: score
    mode: gte
"#,
        );

        let error = predicate(
            &spec,
            Some(Value::Json(json!({"score": 8}))),
            Path::new("."),
        )
        .unwrap_err();

        assert!(error
            .to_string()
            .contains("predicate mode `gte` requires value"));
    }
}
