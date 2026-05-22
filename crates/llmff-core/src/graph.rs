use std::collections::BTreeSet;

use crate::error::LlmffError;
use crate::manifest::{Manifest, StageSpec};

#[derive(Debug, Clone, PartialEq)]
pub struct Graph {
    stages: Vec<StageSpec>,
}

impl Graph {
    pub fn from_manifest(manifest: Manifest) -> Result<Self, LlmffError> {
        let input_ids = manifest.inputs.keys().cloned().collect::<BTreeSet<_>>();
        let mut stage_ids = BTreeSet::new();

        for stage in &manifest.graph {
            if !stage_ids.insert(stage.id.clone()) {
                return Err(LlmffError::GraphValidation(format!(
                    "duplicate stage id `{}`",
                    stage.id
                )));
            }

            if let Some(input) = &stage.input {
                if !input_ids.contains(input) {
                    return Err(LlmffError::GraphValidation(format!(
                        "unknown input reference `{input}`"
                    )));
                }
            }

            if let Some(parent) = &stage.from {
                if !stage_ids.contains(parent) {
                    return Err(LlmffError::GraphValidation(format!(
                        "unknown stage reference `{parent}`"
                    )));
                }
            }
        }

        for output in manifest.outputs.values() {
            if !stage_ids.contains(&output.from) {
                return Err(LlmffError::GraphValidation(format!(
                    "unknown output reference `{}`",
                    output.from
                )));
            }
        }

        Ok(Self {
            stages: manifest.graph,
        })
    }

    pub fn stages(&self) -> &[StageSpec] {
        &self.stages
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Manifest;

    #[test]
    fn validates_stage_references() {
        let manifest = Manifest::from_yaml_str(
            r#"
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
outputs:
  final:
    from: draft
    path: ./answer.json
"#,
        )
        .unwrap();

        let graph = Graph::from_manifest(manifest).expect("graph should validate");

        assert_eq!(graph.stages().len(), 2);
    }

    #[test]
    fn rejects_missing_stage_reference() {
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
graph:
  - id: draft
    op: infer
    from: missing
    model: mock:json
outputs:
  final:
    from: draft
    path: ./answer.json
"#,
        )
        .unwrap();

        let error = Graph::from_manifest(manifest).unwrap_err().to_string();

        assert!(error.contains("unknown stage reference `missing`"));
    }
}
