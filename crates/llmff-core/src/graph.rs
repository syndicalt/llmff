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
        }

        for stage in &manifest.graph {
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
            validate_tool_stage(stage)?;
            validate_write_stage(stage)?;
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
            stages: order_stages(manifest.graph)?,
        })
    }

    pub fn stages(&self) -> &[StageSpec] {
        &self.stages
    }
}

fn order_stages(stages: Vec<StageSpec>) -> Result<Vec<StageSpec>, LlmffError> {
    let mut ordered = Vec::with_capacity(stages.len());
    let mut completed = BTreeSet::new();
    let mut remaining = stages;

    while !remaining.is_empty() {
        let Some(index) = remaining
            .iter()
            .position(|stage| stage_dependencies(stage).is_subset(&completed))
        else {
            return Err(LlmffError::GraphValidation(
                "cycle detected in graph".to_string(),
            ));
        };

        let stage = remaining.remove(index);
        completed.insert(stage.id.clone());
        ordered.push(stage);
    }

    Ok(ordered)
}

pub(crate) fn stage_dependencies(stage: &StageSpec) -> BTreeSet<String> {
    let mut dependencies = BTreeSet::new();

    if let Some(parent) = &stage.from {
        dependencies.insert(parent.clone());
    }

    for target in [
        stage.on_success.as_deref(),
        stage.on_invalid.as_deref(),
        stage.on_skipped.as_deref(),
        stage.default.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        dependencies.insert(target.to_string());
    }

    for target in stage.cases.values() {
        dependencies.insert(target.clone());
    }

    dependencies
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

fn validate_tool_stage(stage: &StageSpec) -> Result<(), LlmffError> {
    if stage.op != "tool" {
        return Ok(());
    }

    match (&stage.command, &stage.url) {
        (None, None) => Err(LlmffError::GraphValidation(
            "tool requires command or url".to_string(),
        )),
        (Some(_), Some(_)) => Err(LlmffError::GraphValidation(
            "tool cannot define both command and url".to_string(),
        )),
        (Some(command), None) if command.is_empty() => Err(LlmffError::GraphValidation(
            "tool command cannot be empty".to_string(),
        )),
        (None, Some(_)) if stage.method.is_none() => Err(LlmffError::GraphValidation(
            "http tool requires method".to_string(),
        )),
        _ => Ok(()),
    }
}

fn validate_write_stage(stage: &StageSpec) -> Result<(), LlmffError> {
    if stage.op != "write" {
        return Ok(());
    }

    if stage.path.is_some() {
        Ok(())
    } else {
        Err(LlmffError::GraphValidation(
            "write requires path".to_string(),
        ))
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
    fn orders_forward_stage_references_by_dependency() {
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
inputs:
  prompt:
    path: ./question.txt
graph:
  - id: draft
    op: infer
    from: load_prompt
    model: mock:json
  - id: load_prompt
    op: load
    input: prompt
outputs:
  final:
    from: draft
    path: ./answer.json
"#,
        )
        .unwrap();

        let graph = Graph::from_manifest(manifest).expect("forward references should validate");
        let stage_ids = graph
            .stages()
            .iter()
            .map(|stage| stage.id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(stage_ids, vec!["load_prompt", "draft"]);
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
    fn orders_forward_route_targets_before_route_stage() {
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
inputs:
  prompt:
    path: ./question.txt
graph:
  - id: choose
    op: route
    from: validate
    on_success: validate
    on_invalid: repair
  - id: repair
    op: repair
    from: validate
    model: mock:good
  - id: validate
    op: validate_json
    from: draft
    schema: '{"type":"object","required":["answer"]}'
  - id: draft
    op: infer
    from: load_prompt
    model: mock:bad
  - id: load_prompt
    op: load
    input: prompt
outputs:
  final:
    from: choose
    path: ./answer.json
"#,
        )
        .unwrap();

        let graph = Graph::from_manifest(manifest).expect("forward route targets should validate");
        let stage_ids = graph
            .stages()
            .iter()
            .map(|stage| stage.id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            stage_ids,
            vec!["load_prompt", "draft", "validate", "repair", "choose"]
        );
    }

    #[test]
    fn rejects_stage_reference_cycle() {
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
graph:
  - id: first
    op: template
    from: second
    path: prompt.tmpl
  - id: second
    op: template
    from: first
    path: prompt.tmpl
"#,
        )
        .unwrap();

        let error = Graph::from_manifest(manifest).unwrap_err().to_string();

        assert!(error.contains("cycle detected in graph"));
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

    #[test]
    fn rejects_tool_without_transport() {
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
graph:
  - id: source
    op: load
  - id: call_tool
    op: tool
    from: source
outputs:
  final:
    from: call_tool
    path: ./answer.txt
"#,
        )
        .unwrap();

        let error = Graph::from_manifest(manifest).unwrap_err().to_string();

        assert!(error.contains("tool requires command or url"));
    }

    #[test]
    fn rejects_tool_with_command_and_url() {
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
graph:
  - id: source
    op: load
  - id: call_tool
    op: tool
    from: source
    command: ["/bin/cat"]
    method: POST
    url: http://127.0.0.1:8080/process
outputs:
  final:
    from: call_tool
    path: ./answer.txt
"#,
        )
        .unwrap();

        let error = Graph::from_manifest(manifest).unwrap_err().to_string();

        assert!(error.contains("tool cannot define both command and url"));
    }

    #[test]
    fn rejects_tool_with_empty_command() {
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
graph:
  - id: source
    op: load
  - id: call_tool
    op: tool
    from: source
    command: []
outputs:
  final:
    from: call_tool
    path: ./answer.txt
"#,
        )
        .unwrap();

        let error = Graph::from_manifest(manifest).unwrap_err().to_string();

        assert!(error.contains("tool command cannot be empty"));
    }

    #[test]
    fn rejects_tool_url_without_method() {
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
graph:
  - id: source
    op: load
  - id: call_tool
    op: tool
    from: source
    url: http://127.0.0.1:8080/process
outputs:
  final:
    from: call_tool
    path: ./answer.txt
"#,
        )
        .unwrap();

        let error = Graph::from_manifest(manifest).unwrap_err().to_string();

        assert!(error.contains("http tool requires method"));
    }

    #[test]
    fn rejects_write_without_path() {
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
graph:
  - id: source
    op: load
  - id: save
    op: write
    from: source
outputs:
  final:
    from: save
    path: ./answer.txt
"#,
        )
        .unwrap();

        let error = Graph::from_manifest(manifest).unwrap_err().to_string();

        assert!(error.contains("write requires path"));
    }
}
