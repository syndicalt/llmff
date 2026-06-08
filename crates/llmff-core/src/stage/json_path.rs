use serde_json::Value as JsonValue;

pub(crate) fn get_json_path<'a>(value: &'a JsonValue, path: &str) -> Option<&'a JsonValue> {
    if path.is_empty() {
        return Some(value);
    }

    let mut current = value;
    for segment in path.split('.') {
        if segment.is_empty() {
            return None;
        }
        current = match current {
            JsonValue::Object(object) => object.get(segment)?,
            JsonValue::Array(items) => items.get(segment.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }

    Some(current)
}

pub(super) fn clone_json_path(value: &JsonValue, path: &str) -> Option<JsonValue> {
    get_json_path(value, path).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_top_level_field() {
        let value = json!({"score": 7, "label": "ok"});

        assert_eq!(get_json_path(&value, "score"), Some(&json!(7)));
    }

    #[test]
    fn reads_nested_dot_path() {
        let value = json!({"result": {"score": 7, "reason": "valid"}});

        assert_eq!(get_json_path(&value, "result.score"), Some(&json!(7)));
    }

    #[test]
    fn reads_array_index_path() {
        let value = json!({"items": [{"score": 1}, {"score": 9}]});

        assert_eq!(get_json_path(&value, "items.1.score"), Some(&json!(9)));
    }

    #[test]
    fn reads_empty_path_as_root() {
        let value = json!({"score": 7});

        assert_eq!(get_json_path(&value, ""), Some(&value));
    }

    #[test]
    fn reports_missing_path() {
        let value = json!({"result": {"score": 7}});

        assert!(get_json_path(&value, "result.missing").is_none());
    }

    #[test]
    fn rejects_empty_path_segments() {
        let value = json!({"result": {"score": 7}});

        assert!(get_json_path(&value, "result..score").is_none());
    }
}
