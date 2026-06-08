use std::path::Path;

use serde_json::{json, Value as JsonValue};

use crate::error::LlmffError;
use crate::manifest::StageSpec;
use crate::value::{StageStatus, Value};

use super::json_path::get_json_path;
use super::{parse_json_stage_input, render_messages_as_text};

pub(super) fn select(
    spec: &StageSpec,
    input: Option<Value>,
    _cwd: &Path,
) -> Result<StageStatus, LlmffError> {
    let value = input.ok_or_else(|| LlmffError::StageExecution {
        stage_id: spec.id.clone(),
        message: "select requires input".to_string(),
    })?;
    let source = value_to_json(spec, value)?;
    let candidates_value = match spec.json_path.as_deref() {
        Some(path) => get_json_path(&source, path).ok_or_else(|| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: format!("select path `{path}` was not found"),
        })?,
        None => &source,
    };
    let JsonValue::Array(candidates) = candidates_value else {
        return Err(LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: "select requires an array of candidates".to_string(),
        });
    };
    if candidates.is_empty() {
        return Err(LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: "select requires at least one candidate".to_string(),
        });
    }

    let mode = spec.mode.as_deref().unwrap_or("highest_score");
    let (index, score) = match mode {
        "highest_score" => select_numeric(candidates, score_field(spec), true, spec)?,
        "field_max" => select_numeric(candidates, required_field(spec)?, true, spec)?,
        "field_min" => select_numeric(candidates, required_field(spec)?, false, spec)?,
        "first_success" => select_success(candidates, true, spec)?,
        "last_success" => select_success(candidates, false, spec)?,
        _ => {
            return Err(LlmffError::StageExecution {
                stage_id: spec.id.clone(),
                message:
                    "select mode must be first_success, last_success, highest_score, field_max, or field_min"
                        .to_string(),
            });
        }
    };

    Ok(StageStatus::Success(Value::Json(json!({
        "selected": candidates[index].clone(),
        "metadata": {
            "selected_index": index,
            "mode": mode,
            "score": score
        }
    }))))
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

fn score_field(spec: &StageSpec) -> &str {
    spec.score_field.as_deref().unwrap_or("score")
}

fn required_field(spec: &StageSpec) -> Result<&str, LlmffError> {
    spec.field
        .as_deref()
        .or(spec.score_field.as_deref())
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: "select field_max and field_min require field or score_field".to_string(),
        })
}

fn select_numeric(
    candidates: &[JsonValue],
    path: &str,
    choose_max: bool,
    spec: &StageSpec,
) -> Result<(usize, JsonValue), LlmffError> {
    let mut selected: Option<(usize, f64)> = None;
    for (index, candidate) in candidates.iter().enumerate() {
        let raw = get_json_path(candidate, path).ok_or_else(|| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: format!("select score path `{path}` was not found"),
        })?;
        let value = raw.as_f64().ok_or_else(|| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: format!("select score path `{path}` must be numeric"),
        })?;
        if !value.is_finite() {
            return Err(LlmffError::StageExecution {
                stage_id: spec.id.clone(),
                message: "select score must be finite".to_string(),
            });
        }
        let should_replace = selected.is_none_or(|(_, current)| {
            if choose_max {
                value > current
            } else {
                value < current
            }
        });
        if should_replace {
            selected = Some((index, value));
        }
    }

    let (index, score) = selected.expect("non-empty candidates should select a numeric score");
    Ok((index, json!(score)))
}

fn select_success(
    candidates: &[JsonValue],
    first: bool,
    spec: &StageSpec,
) -> Result<(usize, JsonValue), LlmffError> {
    let iter: Box<dyn Iterator<Item = (usize, &JsonValue)>> = if first {
        Box::new(candidates.iter().enumerate())
    } else {
        Box::new(candidates.iter().enumerate().rev())
    };
    for (index, candidate) in iter {
        if candidate
            .get("status")
            .and_then(JsonValue::as_str)
            .is_some_and(|status| status == "success")
        {
            return Ok((index, JsonValue::Null));
        }
    }

    Err(LlmffError::StageExecution {
        stage_id: spec.id.clone(),
        message: "select found no successful candidate".to_string(),
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
    fn selects_highest_score_with_stable_tie_break() {
        let spec = stage_from_yaml(
            r#"
version: 1
graph:
  - id: winner
    op: select
    from: candidates
    mode: highest_score
    score_field: score
"#,
        );
        let input = Value::Json(json!([
            {"answer": "a", "score": 7},
            {"answer": "b", "score": 9},
            {"answer": "c", "score": 9}
        ]));

        let value = success_json(select(&spec, Some(input), Path::new(".")).unwrap());

        assert_eq!(value["selected"]["answer"], "b");
        assert_eq!(value["metadata"]["selected_index"], 1);
        assert_eq!(value["metadata"]["score"], 9.0);
    }

    #[test]
    fn selects_from_json_path_array() {
        let spec = stage_from_yaml(
            r#"
version: 1
graph:
  - id: winner
    op: select
    from: loop_result
    json_path: iterations
    mode: field_max
    field: score
"#,
        );
        let input = Value::Json(json!({
            "iterations": [
                {"score": 2, "answer": "low"},
                {"score": 5, "answer": "high"}
            ]
        }));

        let value = success_json(select(&spec, Some(input), Path::new(".")).unwrap());

        assert_eq!(value["selected"]["answer"], "high");
        assert_eq!(value["metadata"]["selected_index"], 1);
    }

    #[test]
    fn selects_first_success_from_iteration_records() {
        let spec = stage_from_yaml(
            r#"
version: 1
graph:
  - id: winner
    op: select
    from: loop_result
    json_path: iterations
    mode: first_success
"#,
        );
        let input = Value::Json(json!({
            "iterations": [
                {"status": "invalid", "value": "bad"},
                {"status": "success", "value": "good"}
            ]
        }));

        let value = success_json(select(&spec, Some(input), Path::new(".")).unwrap());

        assert_eq!(value["selected"]["value"], "good");
        assert_eq!(value["metadata"]["selected_index"], 1);
    }

    #[test]
    fn rejects_empty_candidate_array() {
        let spec = stage_from_yaml(
            r#"
version: 1
graph:
  - id: winner
    op: select
    from: candidates
    mode: highest_score
"#,
        );

        let error = select(&spec, Some(Value::Json(json!([]))), Path::new(".")).unwrap_err();

        assert!(error
            .to_string()
            .contains("select requires at least one candidate"));
    }

    #[test]
    fn rejects_missing_score_field() {
        let spec = stage_from_yaml(
            r#"
version: 1
graph:
  - id: winner
    op: select
    from: candidates
    mode: highest_score
    score_field: score
"#,
        );

        let error = select(
            &spec,
            Some(Value::Json(json!([{"answer": "a"}]))),
            Path::new("."),
        )
        .unwrap_err();

        assert!(error
            .to_string()
            .contains("select score path `score` was not found"));
    }
}
