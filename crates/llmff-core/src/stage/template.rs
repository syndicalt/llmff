use std::collections::BTreeMap;
use std::path::Path;

use crate::error::LlmffError;
use crate::manifest::StageSpec;
use crate::value::{Message, StageStatus, Value};

use super::{render_messages_as_text, resolve_path};

pub(super) fn template(
    spec: &StageSpec,
    input: Option<Value>,
    cwd: &Path,
) -> Result<StageStatus, LlmffError> {
    let template_path = spec
        .path
        .as_ref()
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: "template requires path".to_string(),
        })?;
    let source = std::fs::read_to_string(resolve_path(cwd, template_path)).map_err(|error| {
        LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: format!("failed to read template path `{template_path}`: {error}"),
        }
    })?;
    let variables = template_variables(input.unwrap_or_else(|| Value::Text(String::new())));
    let rendered =
        render_template(&source, &variables).map_err(|variable| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: format!("missing template variable `{variable}`"),
        })?;

    Ok(StageStatus::Success(Value::Text(rendered)))
}

fn template_variables(input: Value) -> BTreeMap<String, String> {
    match input {
        Value::Text(text) => BTreeMap::from([("input".to_string(), text)]),
        Value::Messages(messages) => {
            BTreeMap::from([("input".to_string(), render_messages_as_text(&messages))])
        }
        Value::Json(serde_json::Value::Object(object)) => object
            .into_iter()
            .map(|(key, value)| {
                let rendered = match value {
                    serde_json::Value::String(text) => text,
                    other => other.to_string(),
                };
                (key, rendered)
            })
            .collect(),
        Value::Json(other) => BTreeMap::from([("input".to_string(), other.to_string())]),
    }
}

fn render_template(source: &str, variables: &BTreeMap<String, String>) -> Result<String, String> {
    let mut rendered = String::with_capacity(source.len());
    let mut rest = source;

    while let Some(start) = rest.find("{{") {
        let (before, after_start) = rest.split_at(start);
        rendered.push_str(before);
        let after_start = &after_start[2..];
        let Some(end) = after_start.find("}}") else {
            rendered.push_str("{{");
            rendered.push_str(after_start);
            return Ok(rendered);
        };
        let (name, after_end) = after_start.split_at(end);
        let name = name.trim();
        let value = variables.get(name).ok_or_else(|| name.to_string())?;
        rendered.push_str(value);
        rest = &after_end[2..];
    }

    rendered.push_str(rest);
    Ok(rendered)
}

pub(super) fn system(
    spec: &StageSpec,
    input: Option<Value>,
    cwd: &Path,
) -> Result<StageStatus, LlmffError> {
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
        Value::Messages(messages) => render_messages_as_text(&messages),
        Value::Json(json) => json.to_string(),
    };

    Ok(StageStatus::Success(Value::Messages(vec![
        Message {
            role: "system".to_string(),
            content: system_text,
        },
        Message {
            role: "user".to_string(),
            content: input_text,
        },
    ])))
}
