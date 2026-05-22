use std::collections::BTreeMap;

use serde::Deserialize;

use crate::error::LlmffError;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Manifest {
    pub version: u32,
    #[serde(default)]
    pub inputs: BTreeMap<String, InputSpec>,
    #[serde(default)]
    pub graph: Vec<StageSpec>,
    #[serde(default)]
    pub outputs: BTreeMap<String, OutputSpec>,
}

impl Manifest {
    pub fn from_yaml_str(source: &str) -> Result<Self, LlmffError> {
        serde_yaml::from_str(source).map_err(LlmffError::ManifestParse)
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct InputSpec {
    pub path: Option<String>,
    pub format: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct OutputSpec {
    pub from: String,
    pub path: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct StageSpec {
    pub id: String,
    pub op: String,
    pub input: Option<String>,
    pub from: Option<String>,
    pub path: Option<String>,
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub schema: Option<String>,
    pub schema_path: Option<String>,
    pub when: Option<String>,
    pub on_success: Option<String>,
    pub on_invalid: Option<String>,
    pub on_skipped: Option<String>,
    pub field: Option<String>,
    #[serde(default)]
    pub cases: BTreeMap<String, String>,
    pub default: Option<String>,
    pub command: Option<Vec<String>>,
    pub method: Option<String>,
    pub url: Option<String>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_manifest_with_inputs_graph_and_outputs() {
        let yaml = r#"
version: 1
inputs:
  prompt:
    path: ./question.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: load_prompt
    model: mock:json
    temperature: 0.2
outputs:
  final:
    from: draft
    path: ./answer.json
"#;

        let manifest = Manifest::from_yaml_str(yaml).expect("manifest should parse");

        assert_eq!(manifest.version, 1);
        assert_eq!(
            manifest.inputs["prompt"].path.as_deref(),
            Some("./question.txt")
        );
        assert_eq!(manifest.graph[0].id, "load_prompt");
        assert_eq!(manifest.graph[1].op, "infer");
        assert_eq!(manifest.graph[1].model.as_deref(), Some("mock:json"));
        assert_eq!(manifest.outputs["final"].from, "draft");
    }

    #[test]
    fn parses_schema_path() {
        let yaml = r#"
version: 1
graph:
  - id: validate
    op: validate_json
    from: draft
    schema_path: ./answer.schema.json
"#;

        let manifest = Manifest::from_yaml_str(yaml).expect("manifest should parse");

        assert_eq!(
            manifest.graph[0].schema_path.as_deref(),
            Some("./answer.schema.json")
        );
    }

    #[test]
    fn parses_input_format() {
        let yaml = r#"
version: 1
inputs:
  payload:
    path: ./payload.json
    format: json
graph:
  - id: load_payload
    op: load
    input: payload
"#;

        let manifest = Manifest::from_yaml_str(yaml).expect("manifest should parse");

        assert_eq!(manifest.inputs["payload"].format.as_deref(), Some("json"));
    }

    #[test]
    fn parses_route_fields() {
        let yaml = r#"
version: 1
graph:
  - id: choose
    op: route
    from: classify
    on_success: fast_answer
    on_invalid: repair_answer
    on_skipped: fallback_answer
    field: kind
    cases:
      simple: fast_answer
      hard: strong_answer
    default: fallback_answer
"#;

        let manifest = Manifest::from_yaml_str(yaml).expect("manifest should parse");
        let stage = &manifest.graph[0];

        assert_eq!(stage.on_success.as_deref(), Some("fast_answer"));
        assert_eq!(stage.on_invalid.as_deref(), Some("repair_answer"));
        assert_eq!(stage.on_skipped.as_deref(), Some("fallback_answer"));
        assert_eq!(stage.field.as_deref(), Some("kind"));
        assert_eq!(stage.cases["simple"], "fast_answer");
        assert_eq!(stage.cases["hard"], "strong_answer");
        assert_eq!(stage.default.as_deref(), Some("fallback_answer"));
    }

    #[test]
    fn parses_tool_fields() {
        let yaml = r#"
version: 1
graph:
  - id: call_tool
    op: tool
    from: render_prompt
    command: ["/bin/cat"]
    method: POST
    url: http://127.0.0.1:8080/process
    headers:
      content-type: application/json
"#;

        let manifest = Manifest::from_yaml_str(yaml).expect("manifest should parse");
        let stage = &manifest.graph[0];

        assert_eq!(
            stage.command.as_deref(),
            Some(&["/bin/cat".to_string()][..])
        );
        assert_eq!(stage.method.as_deref(), Some("POST"));
        assert_eq!(stage.url.as_deref(), Some("http://127.0.0.1:8080/process"));
        assert_eq!(stage.headers["content-type"], "application/json");
    }
}
