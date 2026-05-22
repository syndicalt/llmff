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
}
