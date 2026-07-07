use std::path::Path;

use serde_json::{Map as JsonMap, Value as JsonValue};

use crate::error::LlmffError;
use crate::manifest::StageSpec;
use crate::value::{StageStatus, Value};

use super::json_path::{clone_json_path, get_json_path};
use super::specs::{AccumulateMode, AccumulateSpec};
use super::{parse_json_stage_input, render_messages_as_text};

pub(crate) fn accumulate(
    spec: &StageSpec,
    input: Option<Value>,
    state: Option<Value>,
    _cwd: &Path,
) -> Result<StageStatus, LlmffError> {
    let current = input.ok_or_else(|| LlmffError::StageExecution {
        stage_id: spec.id.clone(),
        message: "accumulate requires input".to_string(),
    })?;
    let typed = AccumulateSpec::parse(spec).map_err(|message| LlmffError::StageExecution {
        stage_id: spec.id.clone(),
        message,
    })?;
    let current_json = value_to_json(spec, current)?;
    let state_json = state.map(|value| value_to_json(spec, value)).transpose()?;
    let output = match typed.mode {
        AccumulateMode::Append => accumulate_append(spec, &typed, &current_json, state_json)?,
        AccumulateMode::Extend => accumulate_extend(spec, &typed, &current_json, state_json)?,
        AccumulateMode::MergeObject => {
            accumulate_merge_object(spec, &typed, &current_json, state_json)?
        }
    };

    Ok(StageStatus::Success(Value::Json(output)))
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

fn accumulate_append(
    spec: &StageSpec,
    typed: &AccumulateSpec,
    current: &JsonValue,
    state: Option<JsonValue>,
) -> Result<JsonValue, LlmffError> {
    let mut items = state_array(spec, state)?;
    let item = select_current_value(spec, typed, current)?;
    if let Some(dedupe_field) = typed.dedupe_field.as_deref() {
        if let Some(dedupe_value) = get_json_path(&item, dedupe_field).cloned() {
            items.retain(|existing| get_json_path(existing, dedupe_field) != Some(&dedupe_value));
        }
    }
    items.push(item);
    apply_limit(&mut items, typed.limit);
    Ok(JsonValue::Array(items))
}

fn accumulate_extend(
    spec: &StageSpec,
    typed: &AccumulateSpec,
    current: &JsonValue,
    state: Option<JsonValue>,
) -> Result<JsonValue, LlmffError> {
    let mut items = state_array(spec, state)?;
    let selected = select_current_value(spec, typed, current)?;
    let JsonValue::Array(new_items) = selected else {
        return Err(LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: "accumulate extend requires current value to be an array".to_string(),
        });
    };
    items.extend(new_items);
    if let Some(dedupe_field) = typed.dedupe_field.as_deref() {
        items = dedupe_last_by_field(&items, dedupe_field);
    }
    apply_limit(&mut items, typed.limit);
    Ok(JsonValue::Array(items))
}

fn accumulate_merge_object(
    spec: &StageSpec,
    typed: &AccumulateSpec,
    current: &JsonValue,
    state: Option<JsonValue>,
) -> Result<JsonValue, LlmffError> {
    let mut merged = match state {
        Some(JsonValue::Object(object)) => object,
        Some(_) => {
            return Err(LlmffError::StageExecution {
                stage_id: spec.id.clone(),
                message: "accumulate merge_object state must be an object".to_string(),
            });
        }
        None => JsonMap::new(),
    };
    let selected = select_current_value(spec, typed, current)?;
    let JsonValue::Object(current_object) = selected else {
        return Err(LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: "accumulate merge_object requires current value to be an object".to_string(),
        });
    };
    merged.extend(current_object);
    Ok(JsonValue::Object(merged))
}

fn state_array(spec: &StageSpec, state: Option<JsonValue>) -> Result<Vec<JsonValue>, LlmffError> {
    match state {
        Some(JsonValue::Array(items)) => Ok(items),
        Some(_) => Err(LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: "accumulate array state must be an array".to_string(),
        }),
        None => Ok(Vec::new()),
    }
}

fn select_current_value(
    spec: &StageSpec,
    typed: &AccumulateSpec,
    current: &JsonValue,
) -> Result<JsonValue, LlmffError> {
    let Some(path) = typed.path.as_deref() else {
        return Ok(current.clone());
    };
    clone_json_path(current, path).ok_or_else(|| LlmffError::StageExecution {
        stage_id: spec.id.clone(),
        message: format!("accumulate path `{path}` was not found"),
    })
}

fn apply_limit(items: &mut Vec<JsonValue>, limit: Option<usize>) {
    if let Some(limit) = limit {
        if items.len() > limit {
            let remove_count = items.len() - limit;
            items.drain(0..remove_count);
        }
    }
}

fn dedupe_last_by_field(items: &[JsonValue], field: &str) -> Vec<JsonValue> {
    let mut deduped: Vec<JsonValue> = Vec::new();
    for item in items {
        if let Some(value) = get_json_path(item, field) {
            deduped.retain(|existing| get_json_path(existing, field) != Some(value));
        }
        deduped.push(item.clone());
    }
    deduped
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    use crate::manifest::Manifest;

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
    fn append_starts_new_array_without_state() {
        let spec = stage_from_yaml(
            r#"
version: 1
graph:
  - id: history
    op: accumulate
    from: item
    mode: append
"#,
        );

        let status = accumulate(
            &spec,
            Some(Value::Json(json!({"answer": "one"}))),
            None,
            Path::new("."),
        )
        .unwrap();

        assert_eq!(success_json(status), json!([{"answer": "one"}]));
    }

    #[test]
    fn append_adds_selected_current_value_to_state() {
        let spec = stage_from_yaml(
            r#"
version: 1
graph:
  - id: history
    op: accumulate
    from: item
    state_from: previous_history
    mode: append
    json_path: answer
"#,
        );

        let status = accumulate(
            &spec,
            Some(Value::Json(json!({"answer": "two"}))),
            Some(Value::Json(json!(["one"]))),
            Path::new("."),
        )
        .unwrap();

        assert_eq!(success_json(status), json!(["one", "two"]));
    }

    #[test]
    fn extend_appends_current_array_to_state() {
        let spec = stage_from_yaml(
            r#"
version: 1
graph:
  - id: history
    op: accumulate
    from: batch
    state_from: previous_history
    mode: extend
"#,
        );

        let status = accumulate(
            &spec,
            Some(Value::Json(json!(["two", "three"]))),
            Some(Value::Json(json!(["one"]))),
            Path::new("."),
        )
        .unwrap();

        assert_eq!(success_json(status), json!(["one", "two", "three"]));
    }

    #[test]
    fn merge_object_overlays_current_fields() {
        let spec = stage_from_yaml(
            r#"
version: 1
graph:
  - id: merged
    op: accumulate
    from: update
    state_from: previous
    mode: merge_object
"#,
        );

        let status = accumulate(
            &spec,
            Some(Value::Json(json!({"b": 2}))),
            Some(Value::Json(json!({"a": 1, "b": 1}))),
            Path::new("."),
        )
        .unwrap();

        assert_eq!(success_json(status), json!({"a": 1, "b": 2}));
    }

    #[test]
    fn append_applies_limit_and_dedupe_field() {
        let spec = stage_from_yaml(
            r#"
version: 1
graph:
  - id: history
    op: accumulate
    from: item
    state_from: previous_history
    mode: append
    limit: 2
    dedupe_field: id
"#,
        );

        let status = accumulate(
            &spec,
            Some(Value::Json(json!({"id": "a", "value": 3}))),
            Some(Value::Json(json!([
              {"id": "a", "value": 1},
              {"id": "b", "value": 2}
            ]))),
            Path::new("."),
        )
        .unwrap();

        assert_eq!(
            success_json(status),
            json!([{"id": "b", "value": 2}, {"id": "a", "value": 3}])
        );
    }
}
