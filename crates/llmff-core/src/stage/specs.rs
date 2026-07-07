//! Typed per-op configuration, parsed once from the flat [`StageSpec`].
//!
//! Each `XxxSpec::parse` is the single place that reads a given op's raw
//! `StageSpec` fields and decides whether they form a valid configuration.
//! `crate::graph::validate_stage_op_fields` calls `parse` at graph-build time
//! for the ops it already validated before this module existed, and stage
//! execution (the `stage/*.rs` impls and `Engine`'s per-op `execute_*`
//! methods) calls the same `parse` again to build the value it actually
//! runs with. `parse` returns a plain message on failure rather than an
//! `LlmffError` because the two call sites need different error variants
//! for the same text: `GraphValidation` (exit 10) at graph-build time,
//! `StageExecution` (exit 20) at stage-run time. Wrapping happens at each
//! call site, never inside `parse`, so neither caller can change the
//! other's failure kind.
use serde_json::Value as JsonValue;

use crate::manifest::StageSpec;

pub(crate) type SpecError = String;

fn require_from(stage: &StageSpec, op: &str) -> Result<(), SpecError> {
    if stage.from.is_none() {
        Err(format!("{op} requires from"))
    } else {
        Ok(())
    }
}

pub(crate) struct ExtractSpec {
    pub(crate) path: String,
}

impl ExtractSpec {
    pub(crate) fn parse(stage: &StageSpec) -> Result<Self, SpecError> {
        require_from(stage, "extract")?;
        let path = stage
            .json_path
            .clone()
            .or_else(|| stage.field.clone())
            .ok_or_else(|| "extract requires field or json_path".to_string())?;
        Ok(Self { path })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PredicateMode {
    Truthy,
    Exists,
    Equals,
    Gt,
    Gte,
    Lt,
    Lte,
    Contains,
}

impl PredicateMode {
    fn parse(raw: &str) -> Result<Self, SpecError> {
        Ok(match raw {
            "truthy" => Self::Truthy,
            "exists" => Self::Exists,
            "equals" => Self::Equals,
            "gt" => Self::Gt,
            "gte" => Self::Gte,
            "lt" => Self::Lt,
            "lte" => Self::Lte,
            "contains" => Self::Contains,
            other => return Err(format!("predicate mode `{other}` is not supported")),
        })
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Truthy => "truthy",
            Self::Exists => "exists",
            Self::Equals => "equals",
            Self::Gt => "gt",
            Self::Gte => "gte",
            Self::Lt => "lt",
            Self::Lte => "lte",
            Self::Contains => "contains",
        }
    }

    fn requires_value(self) -> bool {
        matches!(
            self,
            Self::Equals | Self::Gt | Self::Gte | Self::Lt | Self::Lte
        )
    }
}

pub(crate) struct PredicateSpec {
    pub(crate) path: Option<String>,
    pub(crate) mode: PredicateMode,
    pub(crate) value: Option<JsonValue>,
}

impl PredicateSpec {
    pub(crate) fn parse(stage: &StageSpec) -> Result<Self, SpecError> {
        require_from(stage, "predicate")?;
        let mode = PredicateMode::parse(stage.mode.as_deref().unwrap_or("truthy"))?;
        if mode.requires_value() && stage.value.is_none() {
            return Err(format!("predicate mode `{}` requires value", mode.as_str()));
        }
        Ok(Self {
            path: stage.json_path.clone().or_else(|| stage.field.clone()),
            mode,
            value: stage.value.clone(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AccumulateMode {
    Append,
    Extend,
    MergeObject,
}

impl AccumulateMode {
    fn parse(raw: &str) -> Result<Self, SpecError> {
        Ok(match raw {
            "append" => Self::Append,
            "extend" => Self::Extend,
            "merge_object" => Self::MergeObject,
            other => return Err(format!("accumulate mode `{other}` is not supported")),
        })
    }
}

pub(crate) struct AccumulateSpec {
    pub(crate) path: Option<String>,
    pub(crate) mode: AccumulateMode,
    pub(crate) dedupe_field: Option<String>,
    pub(crate) limit: Option<usize>,
}

impl AccumulateSpec {
    pub(crate) fn parse(stage: &StageSpec) -> Result<Self, SpecError> {
        require_from(stage, "accumulate")?;
        let mode = AccumulateMode::parse(stage.mode.as_deref().unwrap_or("append"))?;
        if mode == AccumulateMode::MergeObject && stage.dedupe_field.is_some() {
            return Err("dedupe_field is only supported for array accumulation".to_string());
        }
        if mode == AccumulateMode::MergeObject && stage.limit.is_some() {
            return Err("limit is only supported for array accumulation".to_string());
        }
        Ok(Self {
            path: stage.json_path.clone().or_else(|| stage.field.clone()),
            mode,
            dedupe_field: stage.dedupe_field.clone(),
            limit: stage.limit,
        })
    }
}

pub(crate) struct ScoreSpec {
    pub(crate) score_path: String,
    pub(crate) reason_field: Option<String>,
    pub(crate) label_field: Option<String>,
    pub(crate) min_score: Option<f64>,
    pub(crate) max_score: Option<f64>,
}

impl ScoreSpec {
    pub(crate) fn parse(stage: &StageSpec) -> Result<Self, SpecError> {
        require_from(stage, "score")?;
        let score_path = stage
            .score_field
            .clone()
            .or_else(|| stage.json_path.clone())
            .or_else(|| stage.field.clone())
            .ok_or_else(|| "score requires score_field, field, or json_path".to_string())?;
        if let (Some(min_score), Some(max_score)) = (stage.min_score, stage.max_score) {
            if min_score > max_score {
                return Err("min_score cannot be greater than max_score".to_string());
            }
        }
        Ok(Self {
            score_path,
            reason_field: stage.reason_field.clone(),
            label_field: stage.label_field.clone(),
            min_score: stage.min_score,
            max_score: stage.max_score,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SelectMode {
    FirstSuccess,
    LastSuccess,
    HighestScore,
    FieldMax,
    FieldMin,
}

pub(crate) struct SelectSpec {
    pub(crate) json_path: Option<String>,
    pub(crate) mode: SelectMode,
    pub(crate) field: Option<String>,
    pub(crate) score_field: Option<String>,
}

impl SelectSpec {
    pub(crate) fn parse(stage: &StageSpec) -> Result<Self, SpecError> {
        require_from(stage, "select")?;
        let mode = match stage.mode.as_deref().unwrap_or("highest_score") {
            "first_success" => SelectMode::FirstSuccess,
            "last_success" => SelectMode::LastSuccess,
            "highest_score" => SelectMode::HighestScore,
            "field_max" => SelectMode::FieldMax,
            "field_min" => SelectMode::FieldMin,
            _ => {
                return Err(
                    "select mode must be first_success, last_success, highest_score, field_max, or field_min"
                        .to_string(),
                )
            }
        };
        if matches!(mode, SelectMode::FieldMax | SelectMode::FieldMin)
            && stage.field.is_none()
            && stage.score_field.is_none()
        {
            return Err("select field_max and field_min require field or score_field".to_string());
        }
        Ok(Self {
            json_path: stage.json_path.clone(),
            mode,
            field: stage.field.clone(),
            score_field: stage.score_field.clone(),
        })
    }
}

/// Which of the three mutually exclusive tool transports a `tool` stage
/// selects. The HTTP transport's own `method`/`url`/`headers` stay on
/// `StageSpec` and are re-read by `execute_http_tool` on every retry
/// attempt (see `execute_http_tool_with_retry`); this enum only needs to
/// carry enough to make the transport *selection* (today read ad hoc from
/// three raw fields) a single validated decision.
pub(crate) enum ToolTransport {
    Command(Vec<String>),
    Http,
    Plugin(String),
}

pub(crate) struct ToolSpec {
    pub(crate) transport: ToolTransport,
}

impl ToolSpec {
    pub(crate) fn parse(stage: &StageSpec) -> Result<Self, SpecError> {
        let transport_count = usize::from(stage.command.is_some())
            + usize::from(stage.url.is_some())
            + usize::from(stage.transport.is_some());
        if transport_count == 0 {
            return Err("tool requires command, url, or plugin transport".to_string());
        }
        if transport_count > 1 {
            return Err("tool cannot define more than one transport".to_string());
        }

        if let Some(command) = &stage.command {
            if command.is_empty() {
                return Err("tool command cannot be empty".to_string());
            }
            return Ok(Self {
                transport: ToolTransport::Command(command.clone()),
            });
        }
        if stage.url.is_some() {
            if stage.method.is_none() {
                return Err("http tool requires method".to_string());
            }
            return Ok(Self {
                transport: ToolTransport::Http,
            });
        }
        let transport = stage
            .transport
            .clone()
            .expect("transport_count guarantees exactly one transport field is set");
        Ok(Self {
            transport: ToolTransport::Plugin(transport),
        })
    }
}

pub(crate) struct WriteSpec {
    pub(crate) path: String,
}

impl WriteSpec {
    pub(crate) fn parse(stage: &StageSpec) -> Result<Self, SpecError> {
        let path = stage
            .path
            .clone()
            .ok_or_else(|| "write requires path".to_string())?;
        Ok(Self { path })
    }
}

// The specs below cover ops `graph::validate_stage_op_fields` does not
// validate today (it no-ops for them; see the dispatcher's match arm), so
// their `parse` is only called at stage-execution time, never from the
// graph-validate call site. Adding a graph-time call for them would move an
// existing execution-time `StageExecution` failure (exit 20) earlier into a
// `GraphValidation` failure (exit 10) for manifests that pass today, which
// `Engine::validate_stage` — not `graph::validate_stage_op_fields` — is
// responsible for today. That check stays where it is.

pub(crate) struct LoadSpec {
    pub(crate) input_name: String,
}

impl LoadSpec {
    pub(crate) fn parse(stage: &StageSpec) -> Result<Self, SpecError> {
        let input_name = stage
            .input
            .clone()
            .ok_or_else(|| "load requires input".to_string())?;
        Ok(Self { input_name })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CachePolicy {
    Read,
    Refresh,
    Bypass,
}

pub(crate) struct CacheSpec {
    pub(crate) path: Option<String>,
    pub(crate) cache_policy: CachePolicy,
}

impl CacheSpec {
    /// Mirrors `execute_cache`'s existing fallback exactly: any policy other
    /// than `refresh`/`bypass` (including one absent or already rejected by
    /// the cross-cutting `validate_execution_options` check) behaves as
    /// `read`. This is a resolution, not a validation — `cache_policy`'s
    /// shape is validated once, for every op, by `validate_execution_options`.
    pub(crate) fn parse(stage: &StageSpec) -> Result<Self, SpecError> {
        let cache_policy = match stage.cache_policy.as_deref() {
            Some("bypass") => CachePolicy::Bypass,
            Some("refresh") => CachePolicy::Refresh,
            _ => CachePolicy::Read,
        };
        Ok(Self {
            path: stage.path.clone(),
            cache_policy,
        })
    }
}

pub(crate) struct SystemSpec {
    pub(crate) path: Option<String>,
}

impl SystemSpec {
    pub(crate) fn parse(stage: &StageSpec) -> Result<Self, SpecError> {
        Ok(Self {
            path: stage.path.clone(),
        })
    }
}

pub(crate) struct TemplateSpec {
    pub(crate) path: String,
}

impl TemplateSpec {
    pub(crate) fn parse(stage: &StageSpec) -> Result<Self, SpecError> {
        let path = stage
            .path
            .clone()
            .ok_or_else(|| "template requires path".to_string())?;
        Ok(Self { path })
    }
}

pub(crate) struct ValidateJsonSpec {
    pub(crate) schema: Option<String>,
    pub(crate) schema_path: Option<String>,
}

impl ValidateJsonSpec {
    pub(crate) fn parse(stage: &StageSpec) -> Result<Self, SpecError> {
        if stage.schema.is_none() && stage.schema_path.is_none() {
            return Err("validate_json requires schema or schema_path".to_string());
        }
        Ok(Self {
            schema: stage.schema.clone(),
            schema_path: stage.schema_path.clone(),
        })
    }
}

pub(crate) struct RouteSpec {
    pub(crate) field: Option<String>,
}

impl RouteSpec {
    pub(crate) fn parse(stage: &StageSpec) -> Result<Self, SpecError> {
        require_from(stage, "route")?;
        Ok(Self {
            field: stage.field.clone(),
        })
    }
}

pub(crate) struct InferSpec {
    pub(crate) model: String,
    pub(crate) system: Option<String>,
    pub(crate) temperature: Option<f32>,
    pub(crate) top_p: Option<f32>,
    pub(crate) max_tokens: Option<u32>,
    pub(crate) seed: Option<u64>,
    pub(crate) response_format: Option<String>,
    pub(crate) stop: Vec<String>,
}

impl InferSpec {
    pub(crate) fn parse(stage: &StageSpec) -> Result<Self, SpecError> {
        let model = stage
            .model
            .clone()
            .ok_or_else(|| "infer requires model".to_string())?;
        Ok(Self {
            model,
            system: stage.system.clone(),
            temperature: stage.temperature,
            top_p: stage.top_p,
            max_tokens: stage.max_tokens,
            seed: stage.seed,
            response_format: stage.response_format.clone(),
            stop: stage.stop.clone(),
        })
    }
}

pub(crate) struct RepairSpec {
    pub(crate) model: String,
    pub(crate) system: Option<String>,
    pub(crate) temperature: Option<f32>,
    pub(crate) top_p: Option<f32>,
    pub(crate) max_tokens: Option<u32>,
    pub(crate) seed: Option<u64>,
    pub(crate) response_format: Option<String>,
    pub(crate) stop: Vec<String>,
}

impl RepairSpec {
    pub(crate) fn parse(stage: &StageSpec) -> Result<Self, SpecError> {
        let model = stage
            .model
            .clone()
            .ok_or_else(|| "repair requires model".to_string())?;
        Ok(Self {
            model,
            system: stage.system.clone(),
            temperature: stage.temperature,
            top_p: stage.top_p,
            max_tokens: stage.max_tokens,
            seed: stage.seed,
            response_format: stage.response_format.clone(),
            stop: stage.stop.clone(),
        })
    }
}

/// Which of the three retrieval transports `retrieve`/`rerank` select: direct
/// scoring (lexical or embedding, done in-process) or delegating to an
/// external `command`. Both ops share this one parse point so the "must be
/// lexical, embedding, or command" message and each op's default (retrieve
/// defaults to lexical, rerank defaults to embedding) live in exactly one
/// place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RetrieveStrategy {
    Lexical,
    Embedding,
    Command,
}

impl RetrieveStrategy {
    fn from_stage(
        stage: &StageSpec,
        operation: &str,
        default: RetrieveStrategy,
    ) -> Result<Self, SpecError> {
        match stage.strategy.as_deref().unwrap_or(default.as_str()) {
            "lexical" => Ok(Self::Lexical),
            "embedding" => Ok(Self::Embedding),
            "command" => Ok(Self::Command),
            strategy => Err(format!(
                "{operation} strategy must be lexical, embedding, or command, got `{strategy}`"
            )),
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Lexical => "lexical",
            Self::Embedding => "embedding",
            Self::Command => "command",
        }
    }
}

#[derive(Debug)]
pub(crate) struct RetrieveSpec {
    pub(crate) strategy: RetrieveStrategy,
    pub(crate) documents: Vec<String>,
    pub(crate) top_k: Option<usize>,
    pub(crate) command: Option<Vec<String>>,
    pub(crate) index: Option<String>,
}

impl RetrieveSpec {
    /// Mirrors `Engine::validate_stage`'s pre-conversion `StageOp::Retrieve`
    /// arm, including check order: documents/command/index/top_k are checked
    /// against the *raw* `strategy` field (string comparisons) before the
    /// strategy string itself is validated last via `RetrieveStrategy::from_stage`.
    /// That order is load-bearing: it's what makes
    /// `validate_manifest_rejects_retrieve_index_for_command_strategy` land on
    /// "index requires embedding strategy" rather than a strategy-validity
    /// error, and it's preserved here unchanged.
    pub(crate) fn parse(stage: &StageSpec) -> Result<Self, SpecError> {
        require_from(stage, "retrieve")?;
        let is_command_strategy = stage.strategy.as_deref() == Some("command");
        if stage.documents.is_empty() && !is_command_strategy {
            return Err("retrieve requires documents".to_string());
        }
        if is_command_strategy && stage.command.is_none() {
            return Err("retrieve command strategy requires command".to_string());
        }
        if stage.index.is_some() && stage.strategy.as_deref() != Some("embedding") {
            return Err("retrieve index requires embedding strategy".to_string());
        }
        if let Some(0) = stage.top_k {
            return Err("retrieve top_k must be greater than 0".to_string());
        }
        let strategy = RetrieveStrategy::from_stage(stage, "retrieve", RetrieveStrategy::Lexical)?;
        Ok(Self {
            strategy,
            documents: stage.documents.clone(),
            top_k: stage.top_k,
            command: stage.command.clone(),
            index: stage.index.clone(),
        })
    }
}

#[derive(Debug)]
pub(crate) struct RerankSpec {
    pub(crate) strategy: RetrieveStrategy,
    pub(crate) top_k: Option<usize>,
    pub(crate) command: Option<Vec<String>>,
}

impl RerankSpec {
    /// Mirrors `Engine::validate_stage`'s pre-conversion `StageOp::Rerank` arm:
    /// command-required and top_k are checked before the strategy string is
    /// validated last, matching `RetrieveSpec::parse`.
    pub(crate) fn parse(stage: &StageSpec) -> Result<Self, SpecError> {
        require_from(stage, "rerank")?;
        let is_command_strategy = stage.strategy.as_deref() == Some("command");
        if is_command_strategy && stage.command.is_none() {
            return Err("rerank command strategy requires command".to_string());
        }
        if let Some(0) = stage.top_k {
            return Err("rerank top_k must be greater than 0".to_string());
        }
        let strategy = RetrieveStrategy::from_stage(stage, "rerank", RetrieveStrategy::Embedding)?;
        Ok(Self {
            strategy,
            top_k: stage.top_k,
            command: stage.command.clone(),
        })
    }
}

#[cfg(test)]
mod retrieval_tests {
    use super::*;
    use crate::manifest::Manifest;

    fn stage_from_yaml(yaml: &str) -> StageSpec {
        let mut manifest = Manifest::from_yaml_str(yaml).expect("manifest should parse");
        manifest.graph.remove(0)
    }

    // The branches below have no coverage anywhere else in the workspace:
    // `Engine::validate_stage`'s tests (crates/llmff-core/src/engine.rs) only
    // exercise "requires from", "requires documents", and the two "strategy
    // must be lexical, embedding, or command" cases for retrieve/rerank.

    #[test]
    fn retrieve_spec_rejects_command_strategy_without_command() {
        let stage = stage_from_yaml(
            r#"
version: 1
graph:
  - id: retrieve_context
    op: retrieve
    from: load_prompt
    documents:
      - docs/rust.txt
    strategy: command
"#,
        );

        let error = RetrieveSpec::parse(&stage).unwrap_err();

        assert_eq!(error, "retrieve command strategy requires command");
    }

    #[test]
    fn retrieve_spec_allows_empty_documents_for_command_strategy() {
        let stage = stage_from_yaml(
            r#"
version: 1
graph:
  - id: retrieve_context
    op: retrieve
    from: load_prompt
    strategy: command
    command: ["/bin/cat"]
"#,
        );

        let typed =
            RetrieveSpec::parse(&stage).expect("command strategy without documents should parse");

        assert!(typed.documents.is_empty());
        assert_eq!(typed.strategy, RetrieveStrategy::Command);
    }

    #[test]
    fn retrieve_spec_rejects_zero_top_k() {
        let stage = stage_from_yaml(
            r#"
version: 1
graph:
  - id: retrieve_context
    op: retrieve
    from: load_prompt
    documents:
      - docs/rust.txt
    top_k: 0
"#,
        );

        let error = RetrieveSpec::parse(&stage).unwrap_err();

        assert_eq!(error, "retrieve top_k must be greater than 0");
    }

    #[test]
    fn rerank_spec_rejects_missing_from() {
        let stage = stage_from_yaml(
            r#"
version: 1
graph:
  - id: rerank_context
    op: rerank
"#,
        );

        let error = RerankSpec::parse(&stage).unwrap_err();

        assert_eq!(error, "rerank requires from");
    }

    #[test]
    fn rerank_spec_rejects_command_strategy_without_command() {
        let stage = stage_from_yaml(
            r#"
version: 1
graph:
  - id: rerank_context
    op: rerank
    from: load_candidates
    strategy: command
"#,
        );

        let error = RerankSpec::parse(&stage).unwrap_err();

        assert_eq!(error, "rerank command strategy requires command");
    }

    #[test]
    fn rerank_spec_rejects_zero_top_k() {
        let stage = stage_from_yaml(
            r#"
version: 1
graph:
  - id: rerank_context
    op: rerank
    from: load_candidates
    top_k: 0
"#,
        );

        let error = RerankSpec::parse(&stage).unwrap_err();

        assert_eq!(error, "rerank top_k must be greater than 0");
    }

    #[test]
    fn rerank_spec_defaults_to_embedding_strategy() {
        let stage = stage_from_yaml(
            r#"
version: 1
graph:
  - id: rerank_context
    op: rerank
    from: load_candidates
"#,
        );

        let typed =
            RerankSpec::parse(&stage).expect("rerank without strategy or command should parse");

        assert_eq!(typed.strategy, RetrieveStrategy::Embedding);
        assert_eq!(typed.command, None);
    }
}
