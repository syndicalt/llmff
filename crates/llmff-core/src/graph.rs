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

            validate_route_targets(stage, &stage_ids)?;
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

fn validate_route_targets(
    stage: &StageSpec,
    stage_ids: &BTreeSet<String>,
) -> Result<(), LlmffError> {
    for target in [
        stage.on_success.as_deref(),
        stage.on_invalid.as_deref(),
        stage.on_skipped.as_deref(),
        stage.default.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        validate_route_target(target, stage_ids)?;
    }

    for target in stage.cases.values() {
        validate_route_target(target, stage_ids)?;
    }

    Ok(())
}

fn validate_route_target(target: &str, stage_ids: &BTreeSet<String>) -> Result<(), LlmffError> {
    if stage_ids.contains(target) {
        Ok(())
    } else {
        Err(LlmffError::GraphValidation(format!(
            "unknown route target `{target}`"
        )))
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

    #[test]
    fn validates_route_targets() {
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
graph:
  - id: success_value
    op: load
  - id: invalid_value
    op: load
  - id: choose
    op: route
    from: success_value
    on_success: success_value
    on_invalid: invalid_value
    cases:
      simple: success_value
    default: invalid_value
outputs:
  final:
    from: choose
    path: ./answer.txt
"#,
        )
        .unwrap();

        let graph = Graph::from_manifest(manifest).expect("graph should validate");

        assert_eq!(graph.stages().len(), 3);
    }

    #[test]
    fn rejects_unknown_route_target() {
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
graph:
  - id: source
    op: load
  - id: choose
    op: route
    from: source
    on_success: missing
outputs:
  final:
    from: choose
    path: ./answer.txt
"#,
        )
        .unwrap();

        let error = Graph::from_manifest(manifest).unwrap_err().to_string();

        assert!(error.contains("unknown route target `missing`"));
    }
}
