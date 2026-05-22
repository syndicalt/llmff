use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use jsonschema::JSONSchema;
use serde::Serialize;

use crate::error::LlmffError;
use crate::manifest::StageSpec;
use crate::value::{Message, StageStatus, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct StageMetadata {
    pub name: &'static str,
    pub kind: &'static str,
    pub required_fields: &'static [&'static str],
    pub optional_fields: &'static [&'static str],
    pub capabilities: &'static [&'static str],
}

pub fn builtin_stage_metadata() -> &'static [StageMetadata] {
    &[
        StageMetadata {
            name: "load",
            kind: "input",
            required_fields: &["input"],
            optional_fields: &[],
            capabilities: &["text-input", "json-input", "stdin"],
        },
        StageMetadata {
            name: "cache",
            kind: "storage",
            required_fields: &["from"],
            optional_fields: &["path", "key"],
            capabilities: &["persistent-cache"],
        },
        StageMetadata {
            name: "system",
            kind: "prompt",
            required_fields: &["from"],
            optional_fields: &["path"],
            capabilities: &["chat-messages", "system-prompt"],
        },
        StageMetadata {
            name: "template",
            kind: "prompt",
            required_fields: &["from", "path"],
            optional_fields: &[],
            capabilities: &["file-template", "json-fields"],
        },
        StageMetadata {
            name: "retrieve",
            kind: "retrieval",
            required_fields: &["from", "documents"],
            optional_fields: &["top_k", "strategy"],
            capabilities: &[
                "local-documents",
                "lexical-scoring",
                "local-embedding-scoring",
            ],
        },
        StageMetadata {
            name: "rerank",
            kind: "retrieval",
            required_fields: &["from"],
            optional_fields: &["top_k", "strategy"],
            capabilities: &[
                "retrieval-reranking",
                "lexical-scoring",
                "local-embedding-scoring",
            ],
        },
        StageMetadata {
            name: "infer",
            kind: "model",
            required_fields: &["from", "model"],
            optional_fields: &[
                "temperature",
                "top_p",
                "max_tokens",
                "seed",
                "response_format",
                "stop",
            ],
            capabilities: &[
                "chat-messages",
                "response-format-json",
                "sampling",
                "seed-control",
                "stop-sequences",
                "usage-metadata",
            ],
        },
        StageMetadata {
            name: "validate_json",
            kind: "validation",
            required_fields: &["from", "schema|schema_path"],
            optional_fields: &[],
            capabilities: &["json-schema"],
        },
        StageMetadata {
            name: "repair",
            kind: "model",
            required_fields: &["from", "model"],
            optional_fields: &[
                "temperature",
                "top_p",
                "max_tokens",
                "seed",
                "response_format",
                "stop",
            ],
            capabilities: &[
                "json-repair",
                "response-format-json",
                "sampling",
                "seed-control",
                "stop-sequences",
            ],
        },
        StageMetadata {
            name: "route",
            kind: "control-flow",
            required_fields: &["from", "target"],
            optional_fields: &[
                "on_success",
                "on_invalid",
                "on_skipped",
                "field",
                "cases",
                "default",
            ],
            capabilities: &["status-routing", "json-field-routing"],
        },
        StageMetadata {
            name: "tool",
            kind: "integration",
            required_fields: &["from", "command|url|transport"],
            optional_fields: &["method", "headers"],
            capabilities: &["command-tool", "http-tool", "plugin-tool-transport"],
        },
        StageMetadata {
            name: "write",
            kind: "output",
            required_fields: &["from", "path"],
            optional_fields: &[],
            capabilities: &["file-output", "stdout"],
        },
    ]
}

pub fn execute_deterministic_stage(
    spec: &StageSpec,
    input: Option<Value>,
    cwd: &Path,
) -> Result<StageStatus, LlmffError> {
    match spec.op.as_str() {
        "system" => system(spec, input, cwd),
        "template" => template(spec, input, cwd),
        "retrieve" => retrieve(spec, input, cwd),
        "rerank" => rerank(spec, input),
        "validate_json" => validate_json(spec, input, cwd),
        other => Err(LlmffError::UnknownStage(other.to_string())),
    }
}

fn template(spec: &StageSpec, input: Option<Value>, cwd: &Path) -> Result<StageStatus, LlmffError> {
    let template_path = spec
        .path
        .as_ref()
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: "template requires path".to_string(),
        })?;
    let source = std::fs::read_to_string(resolve_path(cwd, template_path)).map_err(|error| {
        LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: format!("failed to read template path `{template_path}`: {error}"),
        }
    })?;
    let variables = template_variables(input.unwrap_or_else(|| Value::Text(String::new())));
    let rendered =
        render_template(&source, &variables).map_err(|variable| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: format!("missing template variable `{variable}`"),
        })?;

    Ok(StageStatus::Success(Value::Text(rendered)))
}

fn template_variables(input: Value) -> BTreeMap<String, String> {
    match input {
        Value::Text(text) => BTreeMap::from([("input".to_string(), text)]),
        Value::Messages(messages) => {
            BTreeMap::from([("input".to_string(), render_messages_as_text(&messages))])
        }
        Value::Json(serde_json::Value::Object(object)) => object
            .into_iter()
            .map(|(key, value)| {
                let rendered = match value {
                    serde_json::Value::String(text) => text,
                    other => other.to_string(),
                };
                (key, rendered)
            })
            .collect(),
        Value::Json(other) => BTreeMap::from([("input".to_string(), other.to_string())]),
    }
}

fn render_template(source: &str, variables: &BTreeMap<String, String>) -> Result<String, String> {
    let mut rendered = String::with_capacity(source.len());
    let mut rest = source;

    while let Some(start) = rest.find("{{") {
        let (before, after_start) = rest.split_at(start);
        rendered.push_str(before);
        let after_start = &after_start[2..];
        let Some(end) = after_start.find("}}") else {
            rendered.push_str("{{");
            rendered.push_str(after_start);
            return Ok(rendered);
        };
        let (name, after_end) = after_start.split_at(end);
        let name = name.trim();
        let value = variables.get(name).ok_or_else(|| name.to_string())?;
        rendered.push_str(value);
        rest = &after_end[2..];
    }

    rendered.push_str(rest);
    Ok(rendered)
}

fn system(spec: &StageSpec, input: Option<Value>, cwd: &Path) -> Result<StageStatus, LlmffError> {
    let input = input.unwrap_or_else(|| Value::Text(String::new()));
    let Some(system_path) = spec.path.as_ref() else {
        return Ok(StageStatus::Success(input));
    };
    let system_text = std::fs::read_to_string(resolve_path(cwd, system_path)).map_err(|error| {
        LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: format!("failed to read system path `{system_path}`: {error}"),
        }
    })?;
    let input_text = match input {
        Value::Text(text) => text,
        Value::Messages(messages) => render_messages_as_text(&messages),
        Value::Json(json) => json.to_string(),
    };

    Ok(StageStatus::Success(Value::Messages(vec![
        Message {
            role: "system".to_string(),
            content: system_text,
        },
        Message {
            role: "user".to_string(),
            content: input_text,
        },
    ])))
}

fn validate_json(
    spec: &StageSpec,
    input: Option<Value>,
    cwd: &Path,
) -> Result<StageStatus, LlmffError> {
    let value = input.ok_or_else(|| LlmffError::StageExecution {
        stage_id: spec.id.clone(),
        message: "validate_json requires input".to_string(),
    })?;
    let schema_source = schema_source(spec, cwd)?;
    let schema_json: serde_json::Value =
        serde_json::from_str(&schema_source).map_err(|error| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: format!("invalid inline schema: {error}"),
        })?;
    let instance = match &value {
        Value::Json(json) => json.clone(),
        Value::Text(text) => parse_json_stage_input(spec, text)?,
        Value::Messages(messages) => {
            parse_json_stage_input(spec, &render_messages_as_text(messages))?
        }
    };
    let compiled =
        JSONSchema::compile(&schema_json).map_err(|error| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: format!("invalid JSON schema: {error}"),
        })?;

    let validation_errors = compiled
        .validate(&instance)
        .err()
        .map(|errors| errors.map(|error| error.to_string()).collect::<Vec<_>>());

    match validation_errors {
        None => Ok(StageStatus::Success(Value::Json(instance))),
        Some(errors) => Ok(StageStatus::Invalid { value, errors }),
    }
}

fn retrieve(spec: &StageSpec, input: Option<Value>, cwd: &Path) -> Result<StageStatus, LlmffError> {
    let query = input
        .map(render_value_as_text)
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: "retrieve requires input".to_string(),
        })?;
    let strategy = RetrieveStrategy::from_stage(spec, "retrieve", RetrieveStrategy::Lexical)?;
    let query_terms = tokenize(&query);
    let query_embedding = match strategy {
        RetrieveStrategy::Lexical => BTreeMap::new(),
        RetrieveStrategy::Embedding => hashed_embedding(&query),
    };
    let mut matches = Vec::new();

    for document in &spec.documents {
        let text = std::fs::read_to_string(resolve_path(cwd, document)).map_err(|error| {
            LlmffError::StageExecution {
                stage_id: spec.id.clone(),
                message: format!("failed to read retrieve document `{document}`: {error}"),
            }
        })?;
        let score = score_text(strategy, &query_terms, &query_embedding, &text);
        if score.rank > 0.0 {
            matches.push(RetrieveMatch {
                path: document.clone(),
                score,
                text,
            });
        }
    }

    matches.sort_by(|left, right| {
        right
            .score
            .rank
            .partial_cmp(&left.score.rank)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                right
                    .score
                    .lexical_tiebreak
                    .cmp(&left.score.lexical_tiebreak)
            })
            .then_with(|| left.path.cmp(&right.path))
    });
    if let Some(top_k) = spec.top_k {
        matches.truncate(top_k);
    }

    Ok(StageStatus::Success(Value::Json(serde_json::json!({
        "query": query,
        "strategy": strategy.as_str(),
        "matches": matches
            .into_iter()
            .map(|retrieved| {
                serde_json::json!({
                    "path": retrieved.path,
                    "score": retrieved.score.value,
                    "text": retrieved.text,
                })
            })
            .collect::<Vec<_>>(),
    }))))
}

fn rerank(spec: &StageSpec, input: Option<Value>) -> Result<StageStatus, LlmffError> {
    let input = input.ok_or_else(|| LlmffError::StageExecution {
        stage_id: spec.id.clone(),
        message: "rerank requires input".to_string(),
    })?;
    let mut root = match input {
        Value::Json(serde_json::Value::Object(root)) => root,
        _ => {
            return Err(LlmffError::StageExecution {
                stage_id: spec.id.clone(),
                message: "rerank requires JSON object input".to_string(),
            })
        }
    };
    let query = root
        .get("query")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: "rerank requires string query".to_string(),
        })?
        .to_string();
    let matches = root
        .remove("matches")
        .and_then(|value| value.as_array().cloned())
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: "rerank requires matches array".to_string(),
        })?;
    let strategy = RetrieveStrategy::from_stage(spec, "rerank", RetrieveStrategy::Embedding)?;
    let query_terms = tokenize(&query);
    let query_embedding = match strategy {
        RetrieveStrategy::Lexical => BTreeMap::new(),
        RetrieveStrategy::Embedding => hashed_embedding(&query),
    };
    let mut reranked = Vec::new();

    for candidate in matches {
        let serde_json::Value::Object(mut candidate) = candidate else {
            return Err(LlmffError::StageExecution {
                stage_id: spec.id.clone(),
                message: "rerank matches must be objects".to_string(),
            });
        };
        let path = candidate
            .get("path")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| LlmffError::StageExecution {
                stage_id: spec.id.clone(),
                message: "rerank match requires string path".to_string(),
            })?
            .to_string();
        let text = candidate
            .get("text")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| LlmffError::StageExecution {
                stage_id: spec.id.clone(),
                message: "rerank match requires string text".to_string(),
            })?
            .to_string();
        let score = score_text(strategy, &query_terms, &query_embedding, &text);
        candidate.insert("score".to_string(), score.value.clone());
        reranked.push(RerankMatch {
            path,
            score,
            value: serde_json::Value::Object(candidate),
        });
    }

    reranked.sort_by(|left, right| {
        right
            .score
            .rank
            .partial_cmp(&left.score.rank)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                right
                    .score
                    .lexical_tiebreak
                    .cmp(&left.score.lexical_tiebreak)
            })
            .then_with(|| left.path.cmp(&right.path))
    });
    if let Some(top_k) = spec.top_k {
        reranked.truncate(top_k);
    }

    root.insert("query".to_string(), serde_json::Value::String(query));
    root.insert(
        "strategy".to_string(),
        serde_json::Value::String(strategy.as_str().to_string()),
    );
    root.insert(
        "matches".to_string(),
        serde_json::Value::Array(
            reranked
                .into_iter()
                .map(|candidate| candidate.value)
                .collect(),
        ),
    );

    Ok(StageStatus::Success(Value::Json(
        serde_json::Value::Object(root),
    )))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RetrieveStrategy {
    Lexical,
    Embedding,
}

impl RetrieveStrategy {
    fn from_stage(
        spec: &StageSpec,
        operation: &str,
        default: RetrieveStrategy,
    ) -> Result<Self, LlmffError> {
        match spec.strategy.as_deref().unwrap_or(default.as_str()) {
            "lexical" => Ok(Self::Lexical),
            "embedding" => Ok(Self::Embedding),
            strategy => Err(LlmffError::StageExecution {
                stage_id: spec.id.clone(),
                message: format!(
                    "{operation} strategy must be lexical or embedding, got `{strategy}`"
                ),
            }),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Lexical => "lexical",
            Self::Embedding => "embedding",
        }
    }
}

fn score_text(
    strategy: RetrieveStrategy,
    query_terms: &BTreeSet<String>,
    query_embedding: &BTreeMap<String, f64>,
    text: &str,
) -> RetrieveScore {
    match strategy {
        RetrieveStrategy::Lexical => lexical_score(query_terms, &tokenize(text)),
        RetrieveStrategy::Embedding => cosine_similarity(query_embedding, &hashed_embedding(text)),
    }
}

struct RetrieveScore {
    rank: f64,
    lexical_tiebreak: usize,
    value: serde_json::Value,
}

fn lexical_score(
    query_terms: &BTreeSet<String>,
    document_terms: &BTreeSet<String>,
) -> RetrieveScore {
    let score = query_terms
        .iter()
        .filter(|term| document_terms.contains(*term))
        .count();

    RetrieveScore {
        rank: score as f64,
        lexical_tiebreak: score,
        value: serde_json::json!(score),
    }
}

fn embedding_score_value(score: f64) -> serde_json::Value {
    serde_json::json!((score * 1_000_000.0).round() / 1_000_000.0)
}

fn hashed_embedding(source: &str) -> BTreeMap<String, f64> {
    let normalized = source
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    let chars = normalized.chars().collect::<Vec<_>>();
    let mut embedding = BTreeMap::new();

    if chars.is_empty() {
        return embedding;
    }

    if chars.len() < 3 {
        *embedding.entry(normalized).or_insert(0.0) += 1.0;
        return embedding;
    }

    for window in chars.windows(3) {
        let gram = window.iter().collect::<String>();
        *embedding.entry(gram).or_insert(0.0) += 1.0;
    }

    embedding
}

fn cosine_similarity(left: &BTreeMap<String, f64>, right: &BTreeMap<String, f64>) -> RetrieveScore {
    let dot = left
        .iter()
        .filter_map(|(dimension, left_value)| {
            right
                .get(dimension)
                .map(|right_value| left_value * right_value)
        })
        .sum::<f64>();
    let left_norm = vector_norm(left);
    let right_norm = vector_norm(right);
    let score = if left_norm == 0.0 || right_norm == 0.0 {
        0.0
    } else {
        dot / (left_norm * right_norm)
    };

    RetrieveScore {
        rank: score,
        lexical_tiebreak: 0,
        value: embedding_score_value(score),
    }
}

fn vector_norm(vector: &BTreeMap<String, f64>) -> f64 {
    vector
        .values()
        .map(|value| value * value)
        .sum::<f64>()
        .sqrt()
}

struct RetrieveMatch {
    path: String,
    score: RetrieveScore,
    text: String,
}

struct RerankMatch {
    path: String,
    score: RetrieveScore,
    value: serde_json::Value,
}

fn render_value_as_text(value: Value) -> String {
    match value {
        Value::Text(text) => text,
        Value::Messages(messages) => render_messages_as_text(&messages),
        Value::Json(json) => json.to_string(),
    }
}

fn tokenize(source: &str) -> std::collections::BTreeSet<String> {
    source
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(str::to_lowercase)
        .collect()
}

fn parse_json_stage_input(spec: &StageSpec, source: &str) -> Result<serde_json::Value, LlmffError> {
    serde_json::from_str(source).map_err(|error| LlmffError::StageExecution {
        stage_id: spec.id.clone(),
        message: format!("input is not valid JSON: {error}"),
    })
}

fn render_messages_as_text(messages: &[Message]) -> String {
    messages
        .iter()
        .map(|message| format!("{}: {}", message.role, message.content))
        .collect::<Vec<_>>()
        .join("\n")
}

fn schema_source(spec: &StageSpec, cwd: &Path) -> Result<String, LlmffError> {
    if let Some(schema) = &spec.schema {
        return Ok(schema.clone());
    }

    let schema_path = spec
        .schema_path
        .as_ref()
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: "validate_json requires schema or schema_path".to_string(),
        })?;
    let path = resolve_path(cwd, schema_path);
    std::fs::read_to_string(&path).map_err(|error| LlmffError::StageExecution {
        stage_id: spec.id.clone(),
        message: format!("failed to read schema_path `{schema_path}`: {error}"),
    })
}

fn resolve_path(cwd: &Path, path: &str) -> std::path::PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    use crate::manifest::StageSpec;
    use crate::value::{Message, StageStatus, Value};

    #[test]
    fn builtin_stage_metadata_describes_pipeline_operations() {
        let stages = builtin_stage_metadata();

        assert_eq!(stages[0].name, "load");
        assert_eq!(stages[0].kind, "input");
        assert!(stages[0].required_fields.contains(&"input"));

        let infer = stages
            .iter()
            .find(|stage| stage.name == "infer")
            .expect("infer stage should be described");
        assert_eq!(infer.kind, "model");
        assert!(infer.required_fields.contains(&"model"));
        assert!(infer.capabilities.contains(&"sampling"));

        let tool = stages
            .iter()
            .find(|stage| stage.name == "tool")
            .expect("tool stage should be described");
        assert_eq!(tool.kind, "integration");
        assert!(tool.required_fields.contains(&"command|url|transport"));
        assert!(tool.capabilities.contains(&"plugin-tool-transport"));
    }

    #[test]
    fn system_stage_preserves_text_value() {
        let spec = StageSpec {
            id: "policy".to_string(),
            op: "system".to_string(),
            input: None,
            from: Some("load_prompt".to_string()),
            path: None,
            model: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            seed: None,
            response_format: None,
            stop: Vec::new(),
            schema: None,
            schema_path: None,
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            cases: Default::default(),
            default: None,
            command: None,
            transport: None,
            method: None,
            url: None,
            headers: Default::default(),
            documents: Vec::new(),
            top_k: None,
            strategy: None,
            key: None,
        };

        let output = execute_deterministic_stage(
            &spec,
            Some(Value::Text("Explain JSON.".to_string())),
            Path::new("."),
        )
        .expect("stage should run");

        assert_eq!(
            output,
            StageStatus::Success(Value::Text("Explain JSON.".to_string()))
        );
    }

    #[test]
    fn validate_json_marks_invalid_output_without_failing_run() {
        let spec = StageSpec {
            id: "validate".to_string(),
            op: "validate_json".to_string(),
            input: None,
            from: Some("draft".to_string()),
            path: None,
            model: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            seed: None,
            response_format: None,
            stop: Vec::new(),
            schema: Some(r#"{"type":"object","required":["answer"]}"#.to_string()),
            schema_path: None,
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            cases: Default::default(),
            default: None,
            command: None,
            transport: None,
            method: None,
            url: None,
            headers: Default::default(),
            documents: Vec::new(),
            top_k: None,
            strategy: None,
            key: None,
        };

        let output = execute_deterministic_stage(
            &spec,
            Some(Value::Text(r#"{"wrong":true}"#.to_string())),
            Path::new("."),
        )
        .expect("validation stage should run");

        assert!(matches!(output, StageStatus::Invalid { .. }));
    }

    #[test]
    fn validate_json_loads_schema_path() {
        let dir = tempfile::tempdir().unwrap();
        let schema_path = dir.path().join("answer.schema.json");
        std::fs::write(
            &schema_path,
            r#"{"type":"object","required":["answer"],"properties":{"answer":{"type":"string"}}}"#,
        )
        .unwrap();
        let spec = StageSpec {
            id: "validate".to_string(),
            op: "validate_json".to_string(),
            input: None,
            from: Some("draft".to_string()),
            path: None,
            model: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            seed: None,
            response_format: None,
            stop: Vec::new(),
            schema: None,
            schema_path: Some("answer.schema.json".to_string()),
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            cases: Default::default(),
            default: None,
            command: None,
            transport: None,
            method: None,
            url: None,
            headers: Default::default(),
            documents: Vec::new(),
            top_k: None,
            strategy: None,
            key: None,
        };

        let output = execute_deterministic_stage(
            &spec,
            Some(Value::Text(r#"{"answer":"ok"}"#.to_string())),
            dir.path(),
        )
        .expect("validation stage should run");

        assert_eq!(
            output,
            StageStatus::Success(Value::Json(serde_json::json!({"answer":"ok"})))
        );
    }

    #[test]
    fn validate_json_reports_missing_schema_path() {
        let dir = tempfile::tempdir().unwrap();
        let spec = StageSpec {
            id: "validate".to_string(),
            op: "validate_json".to_string(),
            input: None,
            from: Some("draft".to_string()),
            path: None,
            model: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            seed: None,
            response_format: None,
            stop: Vec::new(),
            schema: None,
            schema_path: Some("missing.schema.json".to_string()),
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            cases: Default::default(),
            default: None,
            command: None,
            transport: None,
            method: None,
            url: None,
            headers: Default::default(),
            documents: Vec::new(),
            top_k: None,
            strategy: None,
            key: None,
        };

        let error = execute_deterministic_stage(
            &spec,
            Some(Value::Text(r#"{"answer":"ok"}"#.to_string())),
            dir.path(),
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("schema_path `missing.schema.json`"));
    }

    #[test]
    fn retrieve_stage_returns_top_lexical_matches() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("docs")).unwrap();
        std::fs::write(
            dir.path().join("docs/rust.txt"),
            "Rust builds reliable graph pipelines.",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("docs/python.txt"),
            "Python scripts are useful for quick notebooks.",
        )
        .unwrap();
        let spec = StageSpec {
            id: "retrieve_context".to_string(),
            op: "retrieve".to_string(),
            input: None,
            from: Some("load_prompt".to_string()),
            path: None,
            model: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            seed: None,
            response_format: None,
            stop: Vec::new(),
            schema: None,
            schema_path: None,
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            cases: Default::default(),
            default: None,
            command: None,
            transport: None,
            method: None,
            url: None,
            headers: Default::default(),
            documents: vec!["docs/python.txt".to_string(), "docs/rust.txt".to_string()],
            top_k: Some(1),
            strategy: None,
            key: None,
        };

        let output = execute_deterministic_stage(
            &spec,
            Some(Value::Text("rust graph".to_string())),
            dir.path(),
        )
        .expect("retrieve stage should run");

        let StageStatus::Success(Value::Json(json)) = output else {
            panic!("retrieve should return JSON");
        };
        assert_eq!(json["query"], "rust graph");
        assert_eq!(json["matches"].as_array().unwrap().len(), 1);
        assert_eq!(json["matches"][0]["path"], "docs/rust.txt");
        assert_eq!(json["matches"][0]["score"], 2);
        assert_eq!(
            json["matches"][0]["text"],
            "Rust builds reliable graph pipelines."
        );
    }

    #[test]
    fn retrieve_stage_supports_local_embedding_strategy() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("docs")).unwrap();
        std::fs::write(
            dir.path().join("docs/trust.txt"),
            "Trust systems keep state.",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("docs/python.txt"),
            "Python notebooks handle tables.",
        )
        .unwrap();
        let spec = StageSpec {
            id: "retrieve_context".to_string(),
            op: "retrieve".to_string(),
            input: None,
            from: Some("load_prompt".to_string()),
            path: None,
            model: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            seed: None,
            response_format: None,
            stop: Vec::new(),
            schema: None,
            schema_path: None,
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            cases: Default::default(),
            default: None,
            command: None,
            transport: None,
            method: None,
            url: None,
            headers: Default::default(),
            documents: vec!["docs/python.txt".to_string(), "docs/trust.txt".to_string()],
            top_k: Some(1),
            strategy: Some("embedding".to_string()),
            key: None,
        };

        let output =
            execute_deterministic_stage(&spec, Some(Value::Text("rust".to_string())), dir.path())
                .expect("embedding retrieve stage should run");

        let StageStatus::Success(Value::Json(json)) = output else {
            panic!("retrieve should return JSON");
        };
        assert_eq!(json["strategy"], "embedding");
        assert_eq!(json["matches"].as_array().unwrap().len(), 1);
        assert_eq!(json["matches"][0]["path"], "docs/trust.txt");
        assert!(json["matches"][0]["score"].as_f64().unwrap() > 0.0);
    }

    #[test]
    fn rerank_stage_reorders_matches_with_embedding_strategy() {
        let spec = StageSpec {
            id: "rerank_context".to_string(),
            op: "rerank".to_string(),
            input: None,
            from: Some("retrieve_context".to_string()),
            path: None,
            model: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            seed: None,
            response_format: None,
            stop: Vec::new(),
            schema: None,
            schema_path: None,
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            cases: Default::default(),
            default: None,
            command: None,
            transport: None,
            method: None,
            url: None,
            headers: Default::default(),
            documents: Vec::new(),
            top_k: Some(1),
            strategy: Some("embedding".to_string()),
            key: None,
        };
        let input = Value::Json(serde_json::json!({
            "query": "rust",
            "strategy": "lexical",
            "matches": [
                {
                    "path": "docs/python.txt",
                    "score": 1,
                    "text": "Python notebooks handle tables."
                },
                {
                    "path": "docs/trust.txt",
                    "score": 0,
                    "text": "Trust systems keep state."
                }
            ]
        }));

        let output = execute_deterministic_stage(&spec, Some(input), Path::new("."))
            .expect("rerank stage should run");

        let StageStatus::Success(Value::Json(json)) = output else {
            panic!("rerank should return JSON");
        };
        assert_eq!(json["query"], "rust");
        assert_eq!(json["strategy"], "embedding");
        assert_eq!(json["matches"].as_array().unwrap().len(), 1);
        assert_eq!(json["matches"][0]["path"], "docs/trust.txt");
        assert_eq!(json["matches"][0]["text"], "Trust systems keep state.");
        assert!(json["matches"][0]["score"].as_f64().unwrap() > 0.0);
    }

    #[test]
    fn system_stage_prepends_file_text() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("policy.md"), "Use terse JSON.").unwrap();
        let spec = StageSpec {
            id: "policy".to_string(),
            op: "system".to_string(),
            input: None,
            from: Some("load_prompt".to_string()),
            path: Some("policy.md".to_string()),
            model: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            seed: None,
            response_format: None,
            stop: Vec::new(),
            schema: None,
            schema_path: None,
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            cases: Default::default(),
            default: None,
            command: None,
            transport: None,
            method: None,
            url: None,
            headers: Default::default(),
            documents: Vec::new(),
            top_k: None,
            strategy: None,
            key: None,
        };

        let output = execute_deterministic_stage(
            &spec,
            Some(Value::Text("Return an object.".to_string())),
            dir.path(),
        )
        .expect("system stage should run");

        assert_eq!(
            output,
            StageStatus::Success(Value::Messages(vec![
                Message {
                    role: "system".to_string(),
                    content: "Use terse JSON.".to_string(),
                },
                Message {
                    role: "user".to_string(),
                    content: "Return an object.".to_string(),
                },
            ]))
        );
    }

    #[test]
    fn system_stage_creates_chat_messages_from_policy_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("policy.md"), "Use terse JSON.").unwrap();
        let spec = StageSpec {
            id: "policy".to_string(),
            op: "system".to_string(),
            input: None,
            from: Some("load_prompt".to_string()),
            path: Some("policy.md".to_string()),
            model: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            seed: None,
            response_format: None,
            stop: Vec::new(),
            schema: None,
            schema_path: None,
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            cases: Default::default(),
            default: None,
            command: None,
            transport: None,
            method: None,
            url: None,
            headers: Default::default(),
            documents: Vec::new(),
            top_k: None,
            strategy: None,
            key: None,
        };

        let output = execute_deterministic_stage(
            &spec,
            Some(Value::Text("Return an answer.".to_string())),
            dir.path(),
        )
        .expect("system stage should run");

        assert_eq!(
            output,
            StageStatus::Success(Value::Messages(vec![
                Message {
                    role: "system".to_string(),
                    content: "Use terse JSON.".to_string(),
                },
                Message {
                    role: "user".to_string(),
                    content: "Return an answer.".to_string(),
                },
            ]))
        );
    }

    #[test]
    fn template_stage_substitutes_text_parent_as_input() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("prompt.tmpl"), "Request: {{input}}").unwrap();
        let spec = StageSpec {
            id: "render".to_string(),
            op: "template".to_string(),
            input: None,
            from: Some("load_prompt".to_string()),
            path: Some("prompt.tmpl".to_string()),
            model: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            seed: None,
            response_format: None,
            stop: Vec::new(),
            schema: None,
            schema_path: None,
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            cases: Default::default(),
            default: None,
            command: None,
            transport: None,
            method: None,
            url: None,
            headers: Default::default(),
            documents: Vec::new(),
            top_k: None,
            strategy: None,
            key: None,
        };

        let output = execute_deterministic_stage(
            &spec,
            Some(Value::Text("Return JSON.".to_string())),
            dir.path(),
        )
        .expect("template stage should run");

        assert_eq!(
            output,
            StageStatus::Success(Value::Text("Request: Return JSON.".to_string()))
        );
    }

    #[test]
    fn template_stage_substitutes_json_object_fields() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("prompt.tmpl"),
            "Name: {{name}}, Count: {{count}}, Enabled: {{enabled}}",
        )
        .unwrap();
        let spec = StageSpec {
            id: "render".to_string(),
            op: "template".to_string(),
            input: None,
            from: Some("load_prompt".to_string()),
            path: Some("prompt.tmpl".to_string()),
            model: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            seed: None,
            response_format: None,
            stop: Vec::new(),
            schema: None,
            schema_path: None,
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            cases: Default::default(),
            default: None,
            command: None,
            transport: None,
            method: None,
            url: None,
            headers: Default::default(),
            documents: Vec::new(),
            top_k: None,
            strategy: None,
            key: None,
        };

        let output = execute_deterministic_stage(
            &spec,
            Some(Value::Json(serde_json::json!({
                "name": "Ada",
                "count": 3,
                "enabled": true
            }))),
            dir.path(),
        )
        .expect("template stage should run");

        assert_eq!(
            output,
            StageStatus::Success(Value::Text(
                "Name: Ada, Count: 3, Enabled: true".to_string()
            ))
        );
    }

    #[test]
    fn template_stage_reports_missing_variable() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("prompt.tmpl"), "Request: {{missing}}").unwrap();
        let spec = StageSpec {
            id: "render".to_string(),
            op: "template".to_string(),
            input: None,
            from: Some("load_prompt".to_string()),
            path: Some("prompt.tmpl".to_string()),
            model: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            seed: None,
            response_format: None,
            stop: Vec::new(),
            schema: None,
            schema_path: None,
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            cases: Default::default(),
            default: None,
            command: None,
            transport: None,
            method: None,
            url: None,
            headers: Default::default(),
            documents: Vec::new(),
            top_k: None,
            strategy: None,
            key: None,
        };

        let error = execute_deterministic_stage(
            &spec,
            Some(Value::Text("Return JSON.".to_string())),
            dir.path(),
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("missing template variable `missing`"));
    }
}
