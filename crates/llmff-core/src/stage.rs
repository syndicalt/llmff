use std::path::Path;

use jsonschema::JSONSchema;

use crate::error::LlmffError;
use crate::manifest::StageSpec;
use crate::value::{StageStatus, Value};

pub fn execute_deterministic_stage(
    spec: &StageSpec,
    input: Option<Value>,
    _cwd: &Path,
) -> Result<StageStatus, LlmffError> {
    match spec.op.as_str() {
        "system" => Ok(StageStatus::Success(
            input.unwrap_or_else(|| Value::Text(String::new())),
        )),
        "validate_json" => validate_json(spec, input),
        other => Err(LlmffError::UnknownStage(other.to_string())),
    }
}

fn validate_json(spec: &StageSpec, input: Option<Value>) -> Result<StageStatus, LlmffError> {
    let value = input.ok_or_else(|| LlmffError::StageExecution {
        stage_id: spec.id.clone(),
        message: "validate_json requires input".to_string(),
    })?;
    let schema_source = spec
        .schema
        .as_ref()
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: "validate_json requires schema".to_string(),
        })?;
    let schema_json: serde_json::Value =
        serde_json::from_str(schema_source).map_err(|error| LlmffError::StageExecution {
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
}
