use std::path::Path;

use crate::error::LlmffError;
use crate::manifest::StageSpec;
use crate::value::{StageStatus, Value};

use super::json_path::clone_json_path;
use super::{parse_json_stage_input, render_messages_as_text};

pub(super) fn extract(
    spec: &StageSpec,
    input: Option<Value>,
    _cwd: &Path,
) -> Result<StageStatus, LlmffError> {
    let value = input.ok_or_else(|| LlmffError::StageExecution {
        stage_id: spec.id.clone(),
        message: "extract requires input".to_string(),
    })?;
    let path = spec
        .json_path
        .as_deref()
        .or(spec.field.as_deref())
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: "extract requires field or json_path".to_string(),
        })?;
    let json = match value {
        Value::Json(json) => json,
        Value::Text(text) => parse_json_stage_input(spec, &text)?,
        Value::Messages(messages) => {
            parse_json_stage_input(spec, &render_messages_as_text(&messages))?
        }
    };
    let extracted = clone_json_path(&json, path).ok_or_else(|| LlmffError::StageExecution {
        stage_id: spec.id.clone(),
        message: format!("extract path `{path}` was not found"),
    })?;

    Ok(StageStatus::Success(Value::Json(extracted)))
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

    #[test]
    fn extracts_nested_json_path() {
        let spec = stage_from_yaml(
            r#"
version: 1
graph:
  - id: answer
    op: extract
    from: draft
    json_path: result.final_answer
"#,
        );
        let input = Value::Json(json!({"result": {"final_answer": "done"}}));

        let status = extract(&spec, Some(input), Path::new(".")).unwrap();

        assert_eq!(status, StageStatus::Success(Value::Json(json!("done"))));
    }

    #[test]
    fn extracts_array_index_json_path() {
        let spec = stage_from_yaml(
            r#"
version: 1
graph:
  - id: answer
    op: extract
    from: draft
    json_path: items.1.score
"#,
        );
        let input = Value::Json(json!({"items": [{"score": 1}, {"score": 9}]}));

        let status = extract(&spec, Some(input), Path::new(".")).unwrap();

        assert_eq!(status, StageStatus::Success(Value::Json(json!(9))));
    }

    #[test]
    fn extracts_field_from_text_json() {
        let spec = stage_from_yaml(
            r#"
version: 1
graph:
  - id: answer
    op: extract
    from: draft
    field: answer
"#,
        );
        let input = Value::Text(r#"{"answer":"done"}"#.to_string());

        let status = extract(&spec, Some(input), Path::new(".")).unwrap();

        assert_eq!(status, StageStatus::Success(Value::Json(json!("done"))));
    }

    #[test]
    fn rejects_missing_input() {
        let spec = stage_from_yaml(
            r#"
version: 1
graph:
  - id: answer
    op: extract
    from: draft
    field: answer
"#,
        );

        let error = extract(&spec, None, Path::new(".")).unwrap_err();

        assert!(error.to_string().contains("extract requires input"));
    }

    #[test]
    fn rejects_missing_path() {
        let spec = stage_from_yaml(
            r#"
version: 1
graph:
  - id: answer
    op: extract
    from: draft
    json_path: missing
"#,
        );
        let input = Value::Json(json!({"answer": "done"}));

        let error = extract(&spec, Some(input), Path::new(".")).unwrap_err();

        assert!(error
            .to_string()
            .contains("extract path `missing` was not found"));
    }
}
