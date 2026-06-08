use std::path::Path;

use serde_json::{json, Value as JsonValue};

use crate::error::LlmffError;
use crate::manifest::StageSpec;
use crate::value::{StageStatus, Value};

use super::json_path::get_json_path;
use super::{parse_json_stage_input, render_messages_as_text};

pub(super) fn score(
    spec: &StageSpec,
    input: Option<Value>,
    _cwd: &Path,
) -> Result<StageStatus, LlmffError> {
    let value = input.ok_or_else(|| LlmffError::StageExecution {
        stage_id: spec.id.clone(),
        message: "score requires input".to_string(),
    })?;
    let source = value_to_json(spec, value)?;
    let score_path = score_path(spec)?;
    let raw_score =
        get_json_path(&source, score_path).ok_or_else(|| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: format!("score path `{score_path}` was not found"),
        })?;
    let score = raw_score
        .as_f64()
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: format!("score path `{score_path}` must be numeric"),
        })?;
    if !score.is_finite() {
        return Err(LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: "score must be finite".to_string(),
        });
    }
    if let Some(min_score) = spec.min_score {
        if score < min_score {
            return Err(LlmffError::StageExecution {
                stage_id: spec.id.clone(),
                message: format!("score {score} is below min_score {min_score}"),
            });
        }
    }
    if let Some(max_score) = spec.max_score {
        if score > max_score {
            return Err(LlmffError::StageExecution {
                stage_id: spec.id.clone(),
                message: format!("score {score} is above max_score {max_score}"),
            });
        }
    }

    let mut output = serde_json::Map::new();
    output.insert("score".to_string(), json!(score));
    if let Some(reason) = optional_path_value(spec, &source, spec.reason_field.as_deref())? {
        output.insert("reason".to_string(), reason);
    }
    if let Some(label) = optional_path_value(spec, &source, spec.label_field.as_deref())? {
        output.insert("label".to_string(), label);
    }
    output.insert("source".to_string(), source);

    Ok(StageStatus::Success(Value::Json(JsonValue::Object(output))))
}

fn value_to_json(spec: &StageSpec, value: Value) -> Result<JsonValue, LlmffError> {
    match value {
        Value::Json(json) => Ok(json),
        Value::Text(text) => parse_json_stage_input(spec, &text),
        Value::Messages(messages) => {
            parse_json_stage_input(spec, &render_messages_as_text(&messages))
        }
    }
}

fn score_path(spec: &StageSpec) -> Result<&str, LlmffError> {
    spec.score_field
        .as_deref()
        .or(spec.json_path.as_deref())
        .or(spec.field.as_deref())
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: "score requires score_field, field, or json_path".to_string(),
        })
}

fn optional_path_value(
    spec: &StageSpec,
    source: &JsonValue,
    path: Option<&str>,
) -> Result<Option<JsonValue>, LlmffError> {
    let Some(path) = path else {
        return Ok(None);
    };
    get_json_path(source, path)
        .cloned()
        .map(Some)
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: format!("score metadata path `{path}` was not found"),
        })
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

    fn success_json(status: StageStatus) -> serde_json::Value {
        match status {
            StageStatus::Success(Value::Json(value)) => value,
            other => panic!("unexpected status: {other:?}"),
        }
    }

    #[test]
    fn normalizes_score_with_reason_label_and_source() {
        let spec = stage_from_yaml(
            r#"
version: 1
graph:
  - id: normalized
    op: score
    from: judge
    score_field: result.score
    reason_field: result.reason
    label_field: result.label
    min_score: 0
    max_score: 10
"#,
        );
        let input = Value::Json(json!({
            "result": {
                "score": 8,
                "reason": "cited evidence",
                "label": "usable"
            }
        }));

        let status = score(&spec, Some(input), Path::new(".")).unwrap();
        let value = success_json(status);

        assert_eq!(value["score"], 8.0);
        assert_eq!(value["reason"], "cited evidence");
        assert_eq!(value["label"], "usable");
        assert_eq!(value["source"]["result"]["score"], 8);
    }

    #[test]
    fn accepts_json_path_as_score_source() {
        let spec = stage_from_yaml(
            r#"
version: 1
graph:
  - id: normalized
    op: score
    from: judge
    json_path: score
"#,
        );

        let status = score(
            &spec,
            Some(Value::Json(json!({"score": 9}))),
            Path::new("."),
        )
        .unwrap();

        assert_eq!(success_json(status)["score"], 9.0);
    }

    #[test]
    fn rejects_missing_score_source() {
        let spec = stage_from_yaml(
            r#"
version: 1
graph:
  - id: normalized
    op: score
    from: judge
"#,
        );

        let error = score(
            &spec,
            Some(Value::Json(json!({"score": 9}))),
            Path::new("."),
        )
        .unwrap_err();

        assert!(error
            .to_string()
            .contains("score requires score_field, field, or json_path"));
    }

    #[test]
    fn rejects_non_numeric_score() {
        let spec = stage_from_yaml(
            r#"
version: 1
graph:
  - id: normalized
    op: score
    from: judge
    score_field: score
"#,
        );

        let error = score(
            &spec,
            Some(Value::Json(json!({"score": "high"}))),
            Path::new("."),
        )
        .unwrap_err();

        assert!(error
            .to_string()
            .contains("score path `score` must be numeric"));
    }

    #[test]
    fn rejects_out_of_bounds_score() {
        let spec = stage_from_yaml(
            r#"
version: 1
graph:
  - id: normalized
    op: score
    from: judge
    score_field: score
    min_score: 0
    max_score: 10
"#,
        );

        let error = score(
            &spec,
            Some(Value::Json(json!({"score": 12}))),
            Path::new("."),
        )
        .unwrap_err();

        assert!(error.to_string().contains("score 12 is above max_score 10"));
    }
}
