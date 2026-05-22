use std::path::Path;

use jsonschema::JSONSchema;

use crate::error::LlmffError;
use crate::manifest::StageSpec;
use crate::value::{StageStatus, Value};

pub fn execute_deterministic_stage(
    spec: &StageSpec,
    input: Option<Value>,
    cwd: &Path,
) -> Result<StageStatus, LlmffError> {
    match spec.op.as_str() {
        "system" => system(spec, input, cwd),
        "validate_json" => validate_json(spec, input, cwd),
        other => Err(LlmffError::UnknownStage(other.to_string())),
    }
}

fn system(spec: &StageSpec, input: Option<Value>, cwd: &Path) -> Result<StageStatus, LlmffError> {
    let input = input.unwrap_or_else(|| Value::Text(String::new()));
    let Some(system_path) = spec.path.as_ref() else {
        return Ok(StageStatus::Success(input));
    };
    let system_text = std::fs::read_to_string(resolve_path(cwd, system_path)).map_err(|error| {
        LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: format!("failed to read system path `{system_path}`: {error}"),
        }
    })?;
    let input_text = match input {
        Value::Text(text) => text,
        Value::Json(json) => json.to_string(),
    };

    Ok(StageStatus::Success(Value::Text(format!(
        "{system_text}\n\n{input_text}"
    ))))
}

fn validate_json(
    spec: &StageSpec,
    input: Option<Value>,
    cwd: &Path,
) -> Result<StageStatus, LlmffError> {
    let value = input.ok_or_else(|| LlmffError::StageExecution {
        stage_id: spec.id.clone(),
        message: "validate_json requires input".to_string(),
    })?;
    let schema_source = schema_source(spec, cwd)?;
    let schema_json: serde_json::Value =
        serde_json::from_str(&schema_source).map_err(|error| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: format!("invalid inline schema: {error}"),
        })?;
    let instance = match &value {
        Value::Json(json) => json.clone(),
        Value::Text(text) => {
            serde_json::from_str(text).map_err(|error| LlmffError::StageExecution {
                stage_id: spec.id.clone(),
                message: format!("input is not valid JSON: {error}"),
            })?
        }
    };
    let compiled =
        JSONSchema::compile(&schema_json).map_err(|error| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: format!("invalid JSON schema: {error}"),
        })?;

    let validation_errors = compiled
        .validate(&instance)
        .err()
        .map(|errors| errors.map(|error| error.to_string()).collect::<Vec<_>>());

    match validation_errors {
        None => Ok(StageStatus::Success(Value::Json(instance))),
        Some(errors) => Ok(StageStatus::Invalid { value, errors }),
    }
}

fn schema_source(spec: &StageSpec, cwd: &Path) -> Result<String, LlmffError> {
    if let Some(schema) = &spec.schema {
        return Ok(schema.clone());
    }

    let schema_path = spec
        .schema_path
        .as_ref()
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: "validate_json requires schema or schema_path".to_string(),
        })?;
    let path = resolve_path(cwd, schema_path);
    std::fs::read_to_string(&path).map_err(|error| LlmffError::StageExecution {
        stage_id: spec.id.clone(),
        message: format!("failed to read schema_path `{schema_path}`: {error}"),
    })
}

fn resolve_path(cwd: &Path, path: &str) -> std::path::PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    use crate::manifest::StageSpec;
    use crate::value::{StageStatus, Value};

    #[test]
    fn system_stage_preserves_text_value() {
        let spec = StageSpec {
            id: "policy".to_string(),
            op: "system".to_string(),
            input: None,
            from: Some("load_prompt".to_string()),
            path: None,
            model: None,
            temperature: None,
            schema: None,
            schema_path: None,
            when: None,
        };

        let output = execute_deterministic_stage(
            &spec,
            Some(Value::Text("Explain JSON.".to_string())),
            Path::new("."),
        )
        .expect("stage should run");

        assert_eq!(
            output,
            StageStatus::Success(Value::Text("Explain JSON.".to_string()))
        );
    }

    #[test]
    fn validate_json_marks_invalid_output_without_failing_run() {
        let spec = StageSpec {
            id: "validate".to_string(),
            op: "validate_json".to_string(),
            input: None,
            from: Some("draft".to_string()),
            path: None,
            model: None,
            temperature: None,
            schema: Some(r#"{"type":"object","required":["answer"]}"#.to_string()),
            schema_path: None,
            when: None,
        };

        let output = execute_deterministic_stage(
            &spec,
            Some(Value::Text(r#"{"wrong":true}"#.to_string())),
            Path::new("."),
        )
        .expect("validation stage should run");

        assert!(matches!(output, StageStatus::Invalid { .. }));
    }

    #[test]
    fn validate_json_loads_schema_path() {
        let dir = tempfile::tempdir().unwrap();
        let schema_path = dir.path().join("answer.schema.json");
        std::fs::write(
            &schema_path,
            r#"{"type":"object","required":["answer"],"properties":{"answer":{"type":"string"}}}"#,
        )
        .unwrap();
        let spec = StageSpec {
            id: "validate".to_string(),
            op: "validate_json".to_string(),
            input: None,
            from: Some("draft".to_string()),
            path: None,
            model: None,
            temperature: None,
            schema: None,
            schema_path: Some("answer.schema.json".to_string()),
            when: None,
        };

        let output = execute_deterministic_stage(
            &spec,
            Some(Value::Text(r#"{"answer":"ok"}"#.to_string())),
            dir.path(),
        )
        .expect("validation stage should run");

        assert_eq!(
            output,
            StageStatus::Success(Value::Json(serde_json::json!({"answer":"ok"})))
        );
    }

    #[test]
    fn validate_json_reports_missing_schema_path() {
        let dir = tempfile::tempdir().unwrap();
        let spec = StageSpec {
            id: "validate".to_string(),
            op: "validate_json".to_string(),
            input: None,
            from: Some("draft".to_string()),
            path: None,
            model: None,
            temperature: None,
            schema: None,
            schema_path: Some("missing.schema.json".to_string()),
            when: None,
        };

        let error = execute_deterministic_stage(
            &spec,
            Some(Value::Text(r#"{"answer":"ok"}"#.to_string())),
            dir.path(),
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("schema_path `missing.schema.json`"));
    }

    #[test]
    fn system_stage_prepends_file_text() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("policy.md"), "Use terse JSON.").unwrap();
        let spec = StageSpec {
            id: "policy".to_string(),
            op: "system".to_string(),
            input: None,
            from: Some("load_prompt".to_string()),
            path: Some("policy.md".to_string()),
            model: None,
            temperature: None,
            schema: None,
            schema_path: None,
            when: None,
        };

        let output = execute_deterministic_stage(
            &spec,
            Some(Value::Text("Return an object.".to_string())),
            dir.path(),
        )
        .expect("system stage should run");

        assert_eq!(
            output,
            StageStatus::Success(Value::Text(
                "Use terse JSON.\n\nReturn an object.".to_string()
            ))
        );
    }
}
