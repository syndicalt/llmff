use std::collections::BTreeMap;
use std::path::Path;

use jsonschema::JSONSchema;

use crate::error::LlmffError;
use crate::manifest::StageSpec;
use crate::value::{Message, StageStatus, Value};

pub fn execute_deterministic_stage(
    spec: &StageSpec,
    input: Option<Value>,
    cwd: &Path,
) -> Result<StageStatus, LlmffError> {
    match spec.op.as_str() {
        "system" => system(spec, input, cwd),
        "template" => template(spec, input, cwd),
        "validate_json" => validate_json(spec, input, cwd),
        other => Err(LlmffError::UnknownStage(other.to_string())),
    }
}

fn template(spec: &StageSpec, input: Option<Value>, cwd: &Path) -> Result<StageStatus, LlmffError> {
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

fn parse_json_stage_input(spec: &StageSpec, source: &str) -> Result<serde_json::Value, LlmffError> {
    serde_json::from_str(source).map_err(|error| LlmffError::StageExecution {
        stage_id: spec.id.clone(),
        message: format!("input is not valid JSON: {error}"),
    })
}

fn render_messages_as_text(messages: &[Message]) -> String {
    messages
        .iter()
        .map(|message| format!("{}: {}", message.role, message.content))
        .collect::<Vec<_>>()
        .join("\n")
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
    use crate::value::{Message, StageStatus, Value};

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
            top_p: None,
            max_tokens: None,
            schema: None,
            schema_path: None,
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            cases: Default::default(),
            default: None,
            command: None,
            method: None,
            url: None,
            headers: Default::default(),
            documents: Vec::new(),
            top_k: None,
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
            top_p: None,
            max_tokens: None,
            schema: Some(r#"{"type":"object","required":["answer"]}"#.to_string()),
            schema_path: None,
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            cases: Default::default(),
            default: None,
            command: None,
            method: None,
            url: None,
            headers: Default::default(),
            documents: Vec::new(),
            top_k: None,
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
            top_p: None,
            max_tokens: None,
            schema: None,
            schema_path: Some("answer.schema.json".to_string()),
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            cases: Default::default(),
            default: None,
            command: None,
            method: None,
            url: None,
            headers: Default::default(),
            documents: Vec::new(),
            top_k: None,
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
            top_p: None,
            max_tokens: None,
            schema: None,
            schema_path: Some("missing.schema.json".to_string()),
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            cases: Default::default(),
            default: None,
            command: None,
            method: None,
            url: None,
            headers: Default::default(),
            documents: Vec::new(),
            top_k: None,
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
            top_p: None,
            max_tokens: None,
            schema: None,
            schema_path: None,
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            cases: Default::default(),
            default: None,
            command: None,
            method: None,
            url: None,
            headers: Default::default(),
            documents: Vec::new(),
            top_k: None,
        };

        let output = execute_deterministic_stage(
            &spec,
            Some(Value::Text("Return an object.".to_string())),
            dir.path(),
        )
        .expect("system stage should run");

        assert_eq!(
            output,
            StageStatus::Success(Value::Messages(vec![
                Message {
                    role: "system".to_string(),
                    content: "Use terse JSON.".to_string(),
                },
                Message {
                    role: "user".to_string(),
                    content: "Return an object.".to_string(),
                },
            ]))
        );
    }

    #[test]
    fn system_stage_creates_chat_messages_from_policy_file() {
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
            top_p: None,
            max_tokens: None,
            schema: None,
            schema_path: None,
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            cases: Default::default(),
            default: None,
            command: None,
            method: None,
            url: None,
            headers: Default::default(),
            documents: Vec::new(),
            top_k: None,
        };

        let output = execute_deterministic_stage(
            &spec,
            Some(Value::Text("Return an answer.".to_string())),
            dir.path(),
        )
        .expect("system stage should run");

        assert_eq!(
            output,
            StageStatus::Success(Value::Messages(vec![
                Message {
                    role: "system".to_string(),
                    content: "Use terse JSON.".to_string(),
                },
                Message {
                    role: "user".to_string(),
                    content: "Return an answer.".to_string(),
                },
            ]))
        );
    }

    #[test]
    fn template_stage_substitutes_text_parent_as_input() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("prompt.tmpl"), "Request: {{input}}").unwrap();
        let spec = StageSpec {
            id: "render".to_string(),
            op: "template".to_string(),
            input: None,
            from: Some("load_prompt".to_string()),
            path: Some("prompt.tmpl".to_string()),
            model: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            schema: None,
            schema_path: None,
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            cases: Default::default(),
            default: None,
            command: None,
            method: None,
            url: None,
            headers: Default::default(),
            documents: Vec::new(),
            top_k: None,
        };

        let output = execute_deterministic_stage(
            &spec,
            Some(Value::Text("Return JSON.".to_string())),
            dir.path(),
        )
        .expect("template stage should run");

        assert_eq!(
            output,
            StageStatus::Success(Value::Text("Request: Return JSON.".to_string()))
        );
    }

    #[test]
    fn template_stage_substitutes_json_object_fields() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("prompt.tmpl"),
            "Name: {{name}}, Count: {{count}}, Enabled: {{enabled}}",
        )
        .unwrap();
        let spec = StageSpec {
            id: "render".to_string(),
            op: "template".to_string(),
            input: None,
            from: Some("load_prompt".to_string()),
            path: Some("prompt.tmpl".to_string()),
            model: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            schema: None,
            schema_path: None,
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            cases: Default::default(),
            default: None,
            command: None,
            method: None,
            url: None,
            headers: Default::default(),
            documents: Vec::new(),
            top_k: None,
        };

        let output = execute_deterministic_stage(
            &spec,
            Some(Value::Json(serde_json::json!({
                "name": "Ada",
                "count": 3,
                "enabled": true
            }))),
            dir.path(),
        )
        .expect("template stage should run");

        assert_eq!(
            output,
            StageStatus::Success(Value::Text(
                "Name: Ada, Count: 3, Enabled: true".to_string()
            ))
        );
    }

    #[test]
    fn template_stage_reports_missing_variable() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("prompt.tmpl"), "Request: {{missing}}").unwrap();
        let spec = StageSpec {
            id: "render".to_string(),
            op: "template".to_string(),
            input: None,
            from: Some("load_prompt".to_string()),
            path: Some("prompt.tmpl".to_string()),
            model: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            schema: None,
            schema_path: None,
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            cases: Default::default(),
            default: None,
            command: None,
            method: None,
            url: None,
            headers: Default::default(),
            documents: Vec::new(),
            top_k: None,
        };

        let error = execute_deterministic_stage(
            &spec,
            Some(Value::Text("Return JSON.".to_string())),
            dir.path(),
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("missing template variable `missing`"));
    }
}
