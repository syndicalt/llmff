use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::LlmffError;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Manifest {
    pub version: u32,
    #[serde(default)]
    pub inputs: BTreeMap<String, InputSpec>,
    #[serde(default)]
    pub agents: BTreeMap<String, AgentSpec>,
    #[serde(default)]
    pub graph: Vec<StageSpec>,
    #[serde(default)]
    pub outputs: BTreeMap<String, OutputSpec>,
}

impl Manifest {
    pub fn from_yaml_str(source: &str) -> Result<Self, LlmffError> {
        serde_yaml::from_str(source).map_err(LlmffError::ManifestParse)
    }

    /// Resolve `agent:` references on `infer`/`repair` stages into concrete
    /// inference fields, recursing into loop/map bodies. This is pure
    /// expansion sugar: an agent is a reusable bundle of a persona (`system`)
    /// plus model and sampling settings. Stage-level fields always win over
    /// the referenced agent's defaults, so explicit overrides are preserved.
    ///
    /// llmff does not coordinate agents. This only lets a manifest *name* the
    /// roles in a declared, inspectable topology; the host above still owns
    /// why the pipeline runs and what happens next.
    pub fn resolve_agents(&mut self) -> Result<(), LlmffError> {
        let agents = self.agents.clone();
        for stage in &mut self.graph {
            resolve_stage_agent(stage, &agents)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
pub struct AgentSpec {
    pub model: Option<String>,
    pub system: Option<String>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_tokens: Option<u32>,
    pub seed: Option<u64>,
    #[serde(default)]
    pub stop: Vec<String>,
    pub sampler: Option<String>,
    pub response_format: Option<String>,
}

fn resolve_stage_agent(
    stage: &mut StageSpec,
    agents: &BTreeMap<String, AgentSpec>,
) -> Result<(), LlmffError> {
    if let Some(name) = stage.agent.clone() {
        if !matches!(stage.op.as_str(), "infer" | "repair") {
            return Err(LlmffError::GraphValidation(format!(
                "stage `{}` references agent `{name}` but op `{}` is not `infer` or `repair`",
                stage.id, stage.op
            )));
        }
        let agent = agents.get(&name).ok_or_else(|| {
            LlmffError::GraphValidation(format!(
                "stage `{}` references unknown agent `{name}`",
                stage.id
            ))
        })?;
        apply_agent_defaults(stage, agent);
    }

    for body_stage in &mut stage.body {
        resolve_stage_agent(body_stage, agents)?;
    }

    Ok(())
}

fn apply_agent_defaults(stage: &mut StageSpec, agent: &AgentSpec) {
    if stage.model.is_none() {
        stage.model = agent.model.clone();
    }
    if stage.system.is_none() {
        stage.system = agent.system.clone();
    }
    if stage.temperature.is_none() {
        stage.temperature = agent.temperature;
    }
    if stage.top_p.is_none() {
        stage.top_p = agent.top_p;
    }
    if stage.max_tokens.is_none() {
        stage.max_tokens = agent.max_tokens;
    }
    if stage.seed.is_none() {
        stage.seed = agent.seed;
    }
    if stage.sampler.is_none() {
        stage.sampler = agent.sampler.clone();
    }
    if stage.response_format.is_none() {
        stage.response_format = agent.response_format.clone();
    }
    if stage.stop.is_empty() {
        stage.stop = agent.stop.clone();
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct InputSpec {
    pub path: Option<String>,
    pub format: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct OutputSpec {
    pub from: String,
    pub path: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct StageSpec {
    pub id: String,
    pub op: String,
    pub agent: Option<String>,
    pub system: Option<String>,
    pub input: Option<String>,
    pub from: Option<String>,
    pub state_from: Option<String>,
    pub path: Option<String>,
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_tokens: Option<u32>,
    pub seed: Option<u64>,
    pub response_format: Option<String>,
    #[serde(default)]
    pub stop: Vec<String>,
    pub sampler: Option<String>,
    pub schema: Option<String>,
    pub schema_path: Option<String>,
    pub when: Option<String>,
    pub on_success: Option<String>,
    pub on_invalid: Option<String>,
    pub on_skipped: Option<String>,
    pub field: Option<String>,
    pub json_path: Option<String>,
    pub mode: Option<String>,
    pub criteria: Option<String>,
    pub score_field: Option<String>,
    pub reason_field: Option<String>,
    pub label_field: Option<String>,
    pub min_score: Option<f64>,
    pub max_score: Option<f64>,
    pub value: Option<serde_json::Value>,
    pub limit: Option<usize>,
    pub dedupe_field: Option<String>,
    #[serde(default)]
    pub initial_carry: BTreeMap<String, serde_json::Value>,
    pub items_from: Option<String>,
    pub max_items: Option<usize>,
    pub parallel: Option<bool>,
    pub max_concurrency: Option<usize>,
    #[serde(default)]
    pub cases: BTreeMap<String, String>,
    pub default: Option<String>,
    pub command: Option<Vec<String>>,
    pub transport: Option<String>,
    pub method: Option<String>,
    pub url: Option<String>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub documents: Vec<String>,
    pub top_k: Option<usize>,
    pub strategy: Option<String>,
    pub key: Option<String>,
    pub index: Option<String>,
    pub timeout_ms: Option<u64>,
    pub retry: Option<RetrySpec>,
    pub cache_policy: Option<String>,
    pub max_iterations: Option<usize>,
    pub break_on: Option<LoopBreakSpec>,
    #[serde(default)]
    pub carry: BTreeMap<String, String>,
    #[serde(default, rename = "body")]
    pub body: Vec<StageSpec>,
    #[serde(default, rename = "final")]
    pub final_output: Option<LoopFinalSpec>,
    pub on_iteration_error: Option<String>,
    pub retain_iterations: Option<LoopRetentionSpec>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct RetrySpec {
    pub attempts: usize,
    pub backoff_ms: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LoopBreakSpec {
    StageSuccess {
        stage: String,
    },
    StageFailure {
        stage: String,
    },
    FieldTrue {
        stage: String,
        field: String,
    },
    FieldEquals {
        stage: String,
        field: String,
        value: serde_json::Value,
    },
    Never,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct LoopFinalSpec {
    pub from: String,
    pub require_status: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum LoopRetentionSpec {
    Mode(String),
    Config {
        mode: String,
        #[serde(default)]
        stages: Vec<String>,
        include_values: Option<bool>,
    },
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
    fn parses_sampling_fields() {
        let yaml = r#"
version: 1
graph:
  - id: draft
    op: infer
    from: prompt
    model: mock:good
    temperature: 0.2
    top_p: 0.9
    max_tokens: 256
    seed: 12345
    response_format: json
    stop:
      - "\nEND"
      - "</answer>"
"#;

        let manifest = Manifest::from_yaml_str(yaml).expect("manifest should parse");
        let stage = &manifest.graph[0];

        assert_eq!(stage.temperature, Some(0.2));
        assert_eq!(stage.top_p, Some(0.9));
        assert_eq!(stage.max_tokens, Some(256));
        assert_eq!(stage.seed, Some(12345));
        assert_eq!(stage.response_format.as_deref(), Some("json"));
        assert_eq!(stage.stop, vec!["\nEND", "</answer>"]);
    }

    #[test]
    fn parses_sampler_field() {
        let yaml = r#"
version: 1
graph:
  - id: draft
    op: infer
    from: prompt
    model: mock:good
    sampler: safe-small
"#;

        let manifest = Manifest::from_yaml_str(yaml).expect("manifest should parse");

        assert_eq!(manifest.graph[0].sampler.as_deref(), Some("safe-small"));
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

    #[test]
    fn parses_retrieve_fields() {
        let yaml = r#"
version: 1
graph:
  - id: retrieve_context
    op: retrieve
    from: load_prompt
    documents:
      - docs/rust.txt
      - docs/python.txt
    top_k: 1
    strategy: embedding
    index: .llmff/retrieve/context.index.json
"#;

        let manifest = Manifest::from_yaml_str(yaml).expect("manifest should parse");
        let stage = &manifest.graph[0];

        assert_eq!(stage.documents, vec!["docs/rust.txt", "docs/python.txt"]);
        assert_eq!(stage.top_k, Some(1));
        assert_eq!(stage.strategy.as_deref(), Some("embedding"));
        assert_eq!(
            stage.index.as_deref(),
            Some(".llmff/retrieve/context.index.json")
        );
    }

    #[test]
    fn parses_cache_fields() {
        let yaml = r#"
version: 1
graph:
  - id: cached_prompt
    op: cache
    from: render_prompt
    path: .llmff/cache
    key: prompt-v1
"#;

        let manifest = Manifest::from_yaml_str(yaml).expect("manifest should parse");
        let stage = &manifest.graph[0];

        assert_eq!(stage.path.as_deref(), Some(".llmff/cache"));
        assert_eq!(stage.key.as_deref(), Some("prompt-v1"));
    }

    #[test]
    fn parses_execution_maturity_fields() {
        let yaml = r#"
version: 1
graph:
  - id: draft
    op: infer
    from: prompt
    model: mock:good
    timeout_ms: 1000
    retry:
      attempts: 3
      backoff_ms: 10
  - id: cached
    op: cache
    from: draft
    cache_policy: refresh
"#;

        let manifest = Manifest::from_yaml_str(yaml).expect("manifest should parse");

        assert_eq!(manifest.graph[0].timeout_ms, Some(1000));
        assert_eq!(
            manifest.graph[0].retry.as_ref().map(|retry| retry.attempts),
            Some(3)
        );
        assert_eq!(
            manifest.graph[0]
                .retry
                .as_ref()
                .and_then(|retry| retry.backoff_ms),
            Some(10)
        );
        assert_eq!(manifest.graph[1].cache_policy.as_deref(), Some("refresh"));
    }

    #[test]
    fn parses_loop_stage_fields() {
        let yaml = r#"
version: 1
graph:
  - id: refine
    op: loop
    from: prompt
    max_iterations: 3
    break_on:
      type: stage_success
      stage: check
    carry:
      input: draft
    final:
      from: draft
      require_status: success
    body:
      - id: draft
        op: infer
        from: input
        model: mock:json
      - id: check
        op: validate_json
        from: draft
        schema: '{"type":"object","required":["answer"]}'
"#;

        let manifest = Manifest::from_yaml_str(yaml).expect("manifest should parse");
        let stage = &manifest.graph[0];

        assert_eq!(stage.op, "loop");
        assert_eq!(stage.max_iterations, Some(3));
        assert_eq!(
            stage.break_on.as_ref().unwrap(),
            &LoopBreakSpec::StageSuccess {
                stage: "check".to_string()
            }
        );
        assert_eq!(stage.carry["input"], "draft");
        assert_eq!(
            stage.final_output.as_ref().unwrap(),
            &LoopFinalSpec {
                from: "draft".to_string(),
                require_status: Some("success".to_string())
            }
        );
        assert_eq!(stage.body.len(), 2);
        assert_eq!(stage.body[0].id, "draft");
    }

    #[test]
    fn parses_loop_never_break_condition() {
        let yaml = r#"
version: 1
graph:
  - id: sample
    op: loop
    from: prompt
    max_iterations: 2
    break_on:
      type: never
    body:
      - id: draft
        op: infer
        from: input
        model: mock:json
"#;

        let manifest = Manifest::from_yaml_str(yaml).expect("manifest should parse");
        assert_eq!(manifest.graph[0].break_on, Some(LoopBreakSpec::Never));
    }

    #[test]
    fn parses_extract_json_path_field() {
        let yaml = r#"
version: 1
graph:
  - id: answer
    op: extract
    from: payload
    json_path: result.final_answer
"#;

        let manifest = Manifest::from_yaml_str(yaml).expect("manifest should parse");

        assert_eq!(
            manifest.graph[0].json_path.as_deref(),
            Some("result.final_answer")
        );
    }

    #[test]
    fn resolve_agents_fills_inference_fields_and_recurses_into_bodies() {
        let yaml = r#"
version: 1
agents:
  critic:
    model: mock:good
    system: "Be terse."
    temperature: 0.0
    response_format: json
graph:
  - id: refine
    op: loop
    from: draft
    max_iterations: 2
    body:
      - id: critique
        op: infer
        agent: critic
        from: input
"#;

        let mut manifest = Manifest::from_yaml_str(yaml).expect("manifest should parse");
        manifest.resolve_agents().expect("agents should resolve");

        let critique = &manifest.graph[0].body[0];
        assert_eq!(critique.model.as_deref(), Some("mock:good"));
        assert_eq!(critique.system.as_deref(), Some("Be terse."));
        assert_eq!(critique.temperature, Some(0.0));
        assert_eq!(critique.response_format.as_deref(), Some("json"));
        // The reference is retained so traces and inspect can name the role.
        assert_eq!(critique.agent.as_deref(), Some("critic"));
    }

    #[test]
    fn resolve_agents_lets_stage_fields_override_agent_defaults() {
        let yaml = r#"
version: 1
agents:
  writer:
    model: mock:good
    temperature: 0.7
graph:
  - id: draft
    op: infer
    agent: writer
    from: prompt
    temperature: 0.1
"#;

        let mut manifest = Manifest::from_yaml_str(yaml).expect("manifest should parse");
        manifest.resolve_agents().expect("agents should resolve");

        let draft = &manifest.graph[0];
        assert_eq!(draft.model.as_deref(), Some("mock:good"));
        assert_eq!(draft.temperature, Some(0.1));
    }

    #[test]
    fn resolve_agents_rejects_unknown_agent_reference() {
        let yaml = r#"
version: 1
graph:
  - id: draft
    op: infer
    agent: ghost
    from: prompt
"#;

        let mut manifest = Manifest::from_yaml_str(yaml).expect("manifest should parse");
        let error = manifest.resolve_agents().unwrap_err().to_string();
        assert!(error.contains("unknown agent `ghost`"), "got: {error}");
    }

    #[test]
    fn resolve_agents_rejects_agent_on_non_inference_op() {
        let yaml = r#"
version: 1
agents:
  writer:
    model: mock:good
graph:
  - id: classify
    op: validate_json
    agent: writer
    from: prompt
"#;

        let mut manifest = Manifest::from_yaml_str(yaml).expect("manifest should parse");
        let error = manifest.resolve_agents().unwrap_err().to_string();
        assert!(error.contains("is not `infer` or `repair`"), "got: {error}");
    }

    #[test]
    fn parses_predicate_mode_and_value_fields() {
        let yaml = r#"
version: 1
graph:
  - id: good_enough
    op: predicate
    from: score
    field: value
    mode: gte
    value: 7
"#;

        let manifest = Manifest::from_yaml_str(yaml).expect("manifest should parse");

        assert_eq!(manifest.graph[0].mode.as_deref(), Some("gte"));
        assert_eq!(manifest.graph[0].value, Some(serde_json::json!(7)));
    }
}
