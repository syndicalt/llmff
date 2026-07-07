use std::path::Path;

use jsonschema::JSONSchema;

use crate::error::LlmffError;
use crate::manifest::StageSpec;
use crate::value::{StageStatus, Value};

use super::specs::ValidateJsonSpec;
use super::{parse_json_stage_input, render_messages_as_text, resolve_path};

pub(super) fn validate_json(
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
        Value::Text(text) => parse_json_stage_input(spec, text)?,
        Value::Messages(messages) => {
            parse_json_stage_input(spec, &render_messages_as_text(messages))?
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
    let typed = ValidateJsonSpec::parse(spec).map_err(|message| LlmffError::StageExecution {
        stage_id: spec.id.clone(),
        message,
    })?;
    if let Some(schema) = typed.schema {
        return Ok(schema);
    }

    let schema_path = typed
        .schema_path
        .expect("ValidateJsonSpec::parse guarantees schema or schema_path");
    let path = resolve_path(cwd, &schema_path);
    std::fs::read_to_string(&path).map_err(|error| LlmffError::StageExecution {
        stage_id: spec.id.clone(),
        message: format!("failed to read schema_path `{schema_path}`: {error}"),
    })
}
