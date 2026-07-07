use std::collections::BTreeSet;

use crate::error::LlmffError;
use crate::manifest::{LoopBreakSpec, LoopRetentionSpec, Manifest, StageSpec};
use crate::stage::specs::{
    AccumulateSpec, ExtractSpec, PredicateSpec, ScoreSpec, SelectSpec, ToolSpec, WriteSpec,
};
use crate::stage::StageOp;

#[derive(Debug, Clone, PartialEq)]
pub struct Graph {
    stages: Vec<StageSpec>,
}

impl Graph {
    pub fn from_manifest(mut manifest: Manifest) -> Result<Self, LlmffError> {
        manifest.resolve_agents()?;
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
            if let Some(parent) = &stage.state_from {
                if !stage_ids.contains(parent) {
                    return Err(LlmffError::GraphValidation(format!(
                        "unknown stage reference `{parent}`"
                    )));
                }
            }
            validate_route_targets(stage, &stage_ids)?;
            validate_stage_op_fields(stage)?;
            validate_loop_stage(stage)?;
            validate_map_stage(stage)?;
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
    let stage_ids = stages
        .iter()
        .map(|stage| stage.id.clone())
        .collect::<BTreeSet<_>>();
    let mut remaining = stages;

    while !remaining.is_empty() {
        let Some(index) = remaining
            .iter()
            .position(|stage| stage_graph_dependencies(stage, &stage_ids).is_subset(&completed))
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

pub(crate) fn order_loop_body_stages(stage: &StageSpec) -> Result<Vec<StageSpec>, LlmffError> {
    if stage.op != "loop" {
        return Err(LlmffError::GraphValidation(format!(
            "stage `{}` is not a loop",
            stage.id
        )));
    }
    order_stages(stage.body.clone())
}

pub(crate) fn order_map_body_stages(stage: &StageSpec) -> Result<Vec<StageSpec>, LlmffError> {
    if stage.op != "map" {
        return Err(LlmffError::GraphValidation(format!(
            "stage `{}` is not a map",
            stage.id
        )));
    }
    order_stages(stage.body.clone())
}

pub(crate) fn stage_dependencies(stage: &StageSpec) -> BTreeSet<String> {
    let mut dependencies = BTreeSet::new();

    if let Some(parent) = &stage.from {
        dependencies.insert(parent.clone());
    }
    if let Some(parent) = &stage.state_from {
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

fn stage_graph_dependencies(stage: &StageSpec, stage_ids: &BTreeSet<String>) -> BTreeSet<String> {
    stage_dependencies(stage)
        .into_iter()
        .filter(|dependency| stage_ids.contains(dependency))
        .collect()
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

fn validate_loop_stage(stage: &StageSpec) -> Result<(), LlmffError> {
    if stage.op != "loop" {
        return Ok(());
    }

    if stage.max_iterations.unwrap_or(0) == 0 {
        return Err(LlmffError::GraphValidation(format!(
            "loop `{}` requires max_iterations greater than 0",
            stage.id
        )));
    }
    if stage.body.is_empty() {
        return Err(LlmffError::GraphValidation(format!(
            "loop `{}` requires a non-empty body",
            stage.id
        )));
    }
    if stage.body.iter().any(|body_stage| body_stage.op == "loop") {
        return Err(LlmffError::GraphValidation(
            "nested loop stages are not supported in v1.1".to_string(),
        ));
    }

    let mut body_ids = BTreeSet::new();
    for body_stage in &stage.body {
        if !body_ids.insert(body_stage.id.clone()) {
            return Err(LlmffError::GraphValidation(format!(
                "duplicate loop body stage id `{}`",
                body_stage.id
            )));
        }
    }

    for carry_source in stage.carry.values() {
        if !body_ids.contains(carry_source) {
            return Err(LlmffError::GraphValidation(format!(
                "unknown loop carry source `{carry_source}`"
            )));
        }
    }
    for initial_key in stage.initial_carry.keys() {
        if initial_key == "input" {
            return Err(LlmffError::GraphValidation(
                "initial_carry cannot override reserved loop input".to_string(),
            ));
        }
        if body_ids.contains(initial_key) {
            return Err(LlmffError::GraphValidation(format!(
                "initial_carry key `{initial_key}` collides with loop body stage id"
            )));
        }
        if !stage.carry.contains_key(initial_key) {
            return Err(LlmffError::GraphValidation(format!(
                "initial_carry key `{initial_key}` must match a carry alias"
            )));
        }
    }

    for body_stage in &stage.body {
        if let Some(parent) = &body_stage.from {
            if parent != "input" && !body_ids.contains(parent) && !stage.carry.contains_key(parent)
            {
                return Err(LlmffError::GraphValidation(format!(
                    "unknown loop body reference `{parent}`"
                )));
            }
        }
        if let Some(parent) = &body_stage.state_from {
            if parent != "input" && !body_ids.contains(parent) && !stage.carry.contains_key(parent)
            {
                return Err(LlmffError::GraphValidation(format!(
                    "unknown loop body reference `{parent}`"
                )));
            }
        }
        validate_route_targets(body_stage, &body_ids)?;
        validate_stage_op_fields(body_stage)?;
    }

    if let Some(break_on) = &stage.break_on {
        validate_loop_break_reference(break_on, &body_ids)?;
    } else {
        return Err(LlmffError::GraphValidation(format!(
            "loop `{}` requires break_on",
            stage.id
        )));
    }

    if let Some(final_output) = &stage.final_output {
        if !body_ids.contains(&final_output.from) {
            return Err(LlmffError::GraphValidation(format!(
                "unknown loop final stage `{}`",
                final_output.from
            )));
        }
    }
    validate_loop_retention(stage, &body_ids)?;

    let _ordered_body = order_loop_body_stages(stage)?;
    Ok(())
}

fn validate_map_stage(stage: &StageSpec) -> Result<(), LlmffError> {
    if stage.op != "map" {
        return Ok(());
    }

    if stage.from.is_none() {
        return Err(LlmffError::GraphValidation("map requires from".to_string()));
    }
    if stage.items_from.is_none() {
        return Err(LlmffError::GraphValidation(
            "map requires items_from".to_string(),
        ));
    }
    if stage.max_items.unwrap_or(0) == 0 {
        return Err(LlmffError::GraphValidation(
            "map requires max_items greater than 0".to_string(),
        ));
    }
    if stage.body.is_empty() {
        return Err(LlmffError::GraphValidation(
            "map requires a non-empty body".to_string(),
        ));
    }
    if stage.parallel == Some(true) && stage.max_concurrency.unwrap_or(0) == 0 {
        return Err(LlmffError::GraphValidation(
            "parallel map execution requires max_concurrency greater than 0".to_string(),
        ));
    }
    if stage.parallel != Some(true) && stage.max_concurrency.is_some() {
        return Err(LlmffError::GraphValidation(
            "max_concurrency requires parallel map execution".to_string(),
        ));
    }
    if stage
        .body
        .iter()
        .any(|body_stage| matches!(body_stage.op.as_str(), "loop" | "map"))
    {
        return Err(LlmffError::GraphValidation(
            "nested loop and map stages are not supported in map bodies".to_string(),
        ));
    }

    let mut body_ids = BTreeSet::new();
    for body_stage in &stage.body {
        if !body_ids.insert(body_stage.id.clone()) {
            return Err(LlmffError::GraphValidation(format!(
                "duplicate map body stage id `{}`",
                body_stage.id
            )));
        }
    }

    for body_stage in &stage.body {
        for parent in [body_stage.from.as_ref(), body_stage.state_from.as_ref()]
            .into_iter()
            .flatten()
        {
            if parent != "item" && parent != "input" && !body_ids.contains(parent) {
                return Err(LlmffError::GraphValidation(format!(
                    "unknown map body reference `{parent}`"
                )));
            }
        }
        validate_route_targets(body_stage, &body_ids)?;
        validate_stage_op_fields(body_stage)?;
    }

    if let Some(final_output) = &stage.final_output {
        if !body_ids.contains(&final_output.from) {
            return Err(LlmffError::GraphValidation(format!(
                "unknown map final stage `{}`",
                final_output.from
            )));
        }
    }

    let _ordered_body = order_map_body_stages(stage)?;
    Ok(())
}

fn validate_loop_break_reference(
    break_on: &LoopBreakSpec,
    body_ids: &BTreeSet<String>,
) -> Result<(), LlmffError> {
    let referenced = match break_on {
        LoopBreakSpec::StageSuccess { stage }
        | LoopBreakSpec::StageFailure { stage }
        | LoopBreakSpec::FieldTrue { stage, .. }
        | LoopBreakSpec::FieldEquals { stage, .. } => Some(stage),
        LoopBreakSpec::Never => None,
    };

    if let Some(stage) = referenced {
        if !body_ids.contains(stage) {
            return Err(LlmffError::GraphValidation(format!(
                "unknown loop break stage `{stage}`"
            )));
        }
    }
    Ok(())
}

fn validate_loop_retention(
    stage: &StageSpec,
    body_ids: &BTreeSet<String>,
) -> Result<(), LlmffError> {
    let Some(LoopRetentionSpec::Config { stages, .. }) = &stage.retain_iterations else {
        return Ok(());
    };
    for retained_stage in stages {
        if !body_ids.contains(retained_stage) {
            return Err(LlmffError::GraphValidation(format!(
                "unknown loop retention stage `{retained_stage}`"
            )));
        }
    }
    Ok(())
}

/// Dispatch per-stage-kind field validation by resolved `StageOp`. Exhaustive
/// over every builtin op: a new `StageOp` variant that needs its own field
/// validator (or an explicit "no extra fields to check" decision) must be
/// added here or the crate fails to compile. Unknown and plugin ops are
/// validated elsewhere (engine `validate_stage`); this dispatcher no-ops for
/// them, matching the pre-enum behavior where each per-op validator function
/// silently skipped stages whose `op` didn't match.
fn validate_stage_op_fields(stage: &StageSpec) -> Result<(), LlmffError> {
    let Some(op) = StageOp::from_name(stage.op.as_str()) else {
        return Ok(());
    };

    match op {
        StageOp::Extract => validate_extract_stage(stage),
        StageOp::Predicate => validate_predicate_stage(stage),
        StageOp::Accumulate => validate_accumulate_stage(stage),
        StageOp::Score => validate_score_stage(stage),
        StageOp::Select => validate_select_stage(stage),
        StageOp::Tool => validate_tool_stage(stage),
        StageOp::Write => validate_write_stage(stage),
        StageOp::Load
        | StageOp::Cache
        | StageOp::System
        | StageOp::Template
        | StageOp::Retrieve
        | StageOp::Rerank
        | StageOp::Infer
        | StageOp::ValidateJson
        | StageOp::Repair
        | StageOp::Route
        | StageOp::Loop
        | StageOp::Map => Ok(()),
    }
}

fn validate_extract_stage(stage: &StageSpec) -> Result<(), LlmffError> {
    ExtractSpec::parse(stage)
        .map(|_| ())
        .map_err(LlmffError::GraphValidation)
}

fn validate_predicate_stage(stage: &StageSpec) -> Result<(), LlmffError> {
    PredicateSpec::parse(stage)
        .map(|_| ())
        .map_err(LlmffError::GraphValidation)
}

fn validate_accumulate_stage(stage: &StageSpec) -> Result<(), LlmffError> {
    AccumulateSpec::parse(stage)
        .map(|_| ())
        .map_err(LlmffError::GraphValidation)
}

fn validate_score_stage(stage: &StageSpec) -> Result<(), LlmffError> {
    ScoreSpec::parse(stage)
        .map(|_| ())
        .map_err(LlmffError::GraphValidation)
}

fn validate_select_stage(stage: &StageSpec) -> Result<(), LlmffError> {
    SelectSpec::parse(stage)
        .map(|_| ())
        .map_err(LlmffError::GraphValidation)
}

fn validate_tool_stage(stage: &StageSpec) -> Result<(), LlmffError> {
    ToolSpec::parse(stage)
        .map(|_| ())
        .map_err(LlmffError::GraphValidation)
}

fn validate_write_stage(stage: &StageSpec) -> Result<(), LlmffError> {
    WriteSpec::parse(stage)
        .map(|_| ())
        .map_err(LlmffError::GraphValidation)
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
    fn validates_loop_body_references() {
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
inputs:
  prompt:
    path: ./prompt.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: refine
    op: loop
    from: load_prompt
    max_iterations: 2
    break_on: { type: stage_success, stage: check }
    final: { from: draft, require_status: success }
    body:
      - id: draft
        op: infer
        from: input
        model: mock:json
      - id: check
        op: validate_json
        from: draft
        schema: '{"type":"object"}'
"#,
        )
        .unwrap();

        let graph = Graph::from_manifest(manifest).expect("loop body should validate");
        assert_eq!(graph.stages()[1].id, "refine");
    }

    #[test]
    fn rejects_unknown_loop_break_stage() {
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
inputs:
  prompt:
    path: ./prompt.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: refine
    op: loop
    from: load_prompt
    max_iterations: 2
    break_on: { type: stage_success, stage: missing }
    body:
      - id: draft
        op: infer
        from: input
        model: mock:json
"#,
        )
        .unwrap();

        let error = Graph::from_manifest(manifest).unwrap_err().to_string();
        assert!(error.contains("unknown loop break stage `missing`"));
    }

    #[test]
    fn rejects_unknown_loop_outer_from_reference() {
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
graph:
  - id: refine
    op: loop
    from: missing
    max_iterations: 2
    break_on: { type: never }
    body:
      - id: draft
        op: infer
        from: input
        model: mock:json
"#,
        )
        .unwrap();

        let error = Graph::from_manifest(manifest).unwrap_err().to_string();
        assert!(error.contains("unknown stage reference `missing`"));
    }

    #[test]
    fn rejects_unknown_loop_carry_source() {
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
inputs:
  prompt:
    path: ./prompt.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: refine
    op: loop
    from: load_prompt
    max_iterations: 2
    break_on: { type: never }
    carry:
      input: missing_body_stage
    body:
      - id: draft
        op: infer
        from: input
        model: mock:json
"#,
        )
        .unwrap();

        let error = Graph::from_manifest(manifest).unwrap_err().to_string();
        assert!(error.contains("unknown loop carry source `missing_body_stage`"));
    }

    #[test]
    fn rejects_nested_loop_body_for_v1_1() {
        let manifest = Manifest::from_yaml_str(
            r#"
version: 1
inputs:
  prompt:
    path: ./prompt.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: outer
    op: loop
    from: load_prompt
    max_iterations: 2
    break_on: { type: never }
    body:
      - id: inner
        op: loop
        from: input
        max_iterations: 2
        break_on: { type: never }
        body:
          - id: draft
            op: infer
            from: input
            model: mock:json
"#,
        )
        .unwrap();

        let error = Graph::from_manifest(manifest).unwrap_err().to_string();
        assert!(error.contains("nested loop stages are not supported"));
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

        assert!(error.contains("tool requires command, url, or plugin transport"));
    }

    #[test]
    fn validates_extract_stage_contract() {
        let missing_path = Manifest::from_yaml_str(
            r#"
version: 1
graph:
  - id: source
    op: load
  - id: selected
    op: extract
    from: source
"#,
        )
        .unwrap();
        let error = Graph::from_manifest(missing_path).unwrap_err().to_string();
        assert!(error.contains("extract requires field or json_path"));

        let missing_from = Manifest::from_yaml_str(
            r#"
version: 1
graph:
  - id: selected
    op: extract
    field: answer
"#,
        )
        .unwrap();
        let error = Graph::from_manifest(missing_from).unwrap_err().to_string();
        assert!(error.contains("extract requires from"));
    }

    #[test]
    fn validates_predicate_stage_contract() {
        let missing_value = Manifest::from_yaml_str(
            r#"
version: 1
graph:
  - id: source
    op: load
  - id: ready
    op: predicate
    from: source
    field: score
    mode: gte
"#,
        )
        .unwrap();
        let error = Graph::from_manifest(missing_value).unwrap_err().to_string();
        assert!(error.contains("predicate mode `gte` requires value"));

        let invalid_mode = Manifest::from_yaml_str(
            r#"
version: 1
graph:
  - id: source
    op: load
  - id: ready
    op: predicate
    from: source
    mode: unknown
"#,
        )
        .unwrap();
        let error = Graph::from_manifest(invalid_mode).unwrap_err().to_string();
        assert!(error.contains("predicate mode `unknown` is not supported"));

        let missing_from = Manifest::from_yaml_str(
            r#"
version: 1
graph:
  - id: ready
    op: predicate
    field: score
"#,
        )
        .unwrap();
        let error = Graph::from_manifest(missing_from).unwrap_err().to_string();
        assert!(error.contains("predicate requires from"));
    }

    #[test]
    fn validates_accumulate_stage_contract() {
        let missing_from = Manifest::from_yaml_str(
            r#"
version: 1
graph:
  - id: history
    op: accumulate
"#,
        )
        .unwrap();
        let error = Graph::from_manifest(missing_from).unwrap_err().to_string();
        assert!(error.contains("accumulate requires from"));

        let invalid_mode = Manifest::from_yaml_str(
            r#"
version: 1
graph:
  - id: source
    op: load
  - id: history
    op: accumulate
    from: source
    mode: unknown
"#,
        )
        .unwrap();
        let error = Graph::from_manifest(invalid_mode).unwrap_err().to_string();
        assert!(error.contains("accumulate mode `unknown` is not supported"));

        let dedupe_with_merge = Manifest::from_yaml_str(
            r#"
version: 1
graph:
  - id: source
    op: load
  - id: history
    op: accumulate
    from: source
    mode: merge_object
    dedupe_field: id
"#,
        )
        .unwrap();
        let error = Graph::from_manifest(dedupe_with_merge)
            .unwrap_err()
            .to_string();
        assert!(error.contains("dedupe_field is only supported for array accumulation"));

        let limit_with_merge = Manifest::from_yaml_str(
            r#"
version: 1
graph:
  - id: source
    op: load
  - id: history
    op: accumulate
    from: source
    mode: merge_object
    limit: 2
"#,
        )
        .unwrap();
        let error = Graph::from_manifest(limit_with_merge)
            .unwrap_err()
            .to_string();
        assert!(error.contains("limit is only supported for array accumulation"));
    }

    #[test]
    fn validates_score_stage_contract() {
        let missing_from = Manifest::from_yaml_str(
            r#"
version: 1
graph:
  - id: normalized
    op: score
    score_field: score
"#,
        )
        .unwrap();
        let error = Graph::from_manifest(missing_from).unwrap_err().to_string();
        assert!(error.contains("score requires from"));

        let missing_path = Manifest::from_yaml_str(
            r#"
version: 1
graph:
  - id: source
    op: load
  - id: normalized
    op: score
    from: source
"#,
        )
        .unwrap();
        let error = Graph::from_manifest(missing_path).unwrap_err().to_string();
        assert!(error.contains("score requires score_field, field, or json_path"));

        let inverted_bounds = Manifest::from_yaml_str(
            r#"
version: 1
graph:
  - id: source
    op: load
  - id: normalized
    op: score
    from: source
    score_field: score
    min_score: 10
    max_score: 0
"#,
        )
        .unwrap();
        let error = Graph::from_manifest(inverted_bounds)
            .unwrap_err()
            .to_string();
        assert!(error.contains("min_score cannot be greater than max_score"));
    }

    #[test]
    fn validates_select_stage_contract() {
        let missing_from = Manifest::from_yaml_str(
            r#"
version: 1
graph:
  - id: winner
    op: select
"#,
        )
        .unwrap();
        let error = Graph::from_manifest(missing_from).unwrap_err().to_string();
        assert!(error.contains("select requires from"));

        let invalid_mode = Manifest::from_yaml_str(
            r#"
version: 1
graph:
  - id: source
    op: load
  - id: winner
    op: select
    from: source
    mode: unknown
"#,
        )
        .unwrap();
        let error = Graph::from_manifest(invalid_mode).unwrap_err().to_string();
        assert!(error.contains(
            "select mode must be first_success, last_success, highest_score, field_max, or field_min"
        ));

        let missing_field = Manifest::from_yaml_str(
            r#"
version: 1
graph:
  - id: source
    op: load
  - id: winner
    op: select
    from: source
    mode: field_max
"#,
        )
        .unwrap();
        let error = Graph::from_manifest(missing_field).unwrap_err().to_string();
        assert!(error.contains("select field_max and field_min require field or score_field"));
    }

    #[test]
    fn validates_parallel_map_concurrency_contract() {
        let missing_cap = Manifest::from_yaml_str(
            r#"
version: 1
graph:
  - id: load_payload
    op: load
  - id: names
    op: map
    from: load_payload
    items_from: items
    max_items: 3
    parallel: true
    body:
      - id: name
        op: extract
        from: item
        field: name
"#,
        )
        .unwrap();
        let error = Graph::from_manifest(missing_cap).unwrap_err().to_string();
        assert!(error.contains("parallel map execution requires max_concurrency"));

        let valid = Manifest::from_yaml_str(
            r#"
version: 1
graph:
  - id: load_payload
    op: load
  - id: names
    op: map
    from: load_payload
    items_from: items
    max_items: 3
    parallel: true
    max_concurrency: 2
    body:
      - id: name
        op: extract
        from: item
        field: name
"#,
        )
        .unwrap();
        Graph::from_manifest(valid).expect("bounded parallel map should validate");
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

        assert!(error.contains("tool cannot define more than one transport"));
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
