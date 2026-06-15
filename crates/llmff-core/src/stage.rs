use std::path::Path;

use serde::Serialize;

use crate::error::LlmffError;
use crate::manifest::StageSpec;
use crate::value::{Message, StageStatus, Value};

mod accumulate;
mod extract;
mod json_path;
mod predicate;
mod retrieval;
mod score;
mod select;
mod template;
mod validate;

pub(crate) use accumulate::accumulate;
use extract::extract;
pub(crate) use json_path::get_json_path;
use predicate::predicate;
use retrieval::{rerank, retrieve};
use score::score;
use select::select;
use template::{system, template};
use validate::validate_json;

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
            optional_fields: &["path", "key", "cache_policy"],
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
            optional_fields: &["top_k", "strategy", "command", "index"],
            capabilities: &[
                "command-retrieval",
                "local-documents",
                "lexical-scoring",
                "local-embedding-scoring",
                "persistent-vector-index",
            ],
        },
        StageMetadata {
            name: "rerank",
            kind: "retrieval",
            required_fields: &["from"],
            optional_fields: &["top_k", "strategy", "command"],
            capabilities: &[
                "command-reranking",
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
                "sampler",
                "timeout_ms",
                "retry",
            ],
            capabilities: &[
                "chat-messages",
                "plugin-sampler",
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
            name: "extract",
            kind: "transform",
            required_fields: &["from", "field|json_path"],
            optional_fields: &[],
            capabilities: &["json-field-extraction", "dot-path"],
        },
        StageMetadata {
            name: "predicate",
            kind: "validation",
            required_fields: &["from"],
            optional_fields: &["field", "json_path", "mode", "value"],
            capabilities: &["json-predicate", "loop-break-condition"],
        },
        StageMetadata {
            name: "accumulate",
            kind: "transform",
            required_fields: &["from"],
            optional_fields: &[
                "state_from",
                "field",
                "json_path",
                "mode",
                "limit",
                "dedupe_field",
            ],
            capabilities: &["json-accumulation", "loop-carry-state"],
        },
        StageMetadata {
            name: "score",
            kind: "transform",
            required_fields: &["from", "score_field|field|json_path"],
            optional_fields: &["reason_field", "label_field", "min_score", "max_score"],
            capabilities: &["score-normalization", "json-transform"],
        },
        StageMetadata {
            name: "select",
            kind: "transform",
            required_fields: &["from"],
            optional_fields: &["json_path", "mode", "field", "score_field"],
            capabilities: &["candidate-selection", "best-of-n"],
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
                "sampler",
                "timeout_ms",
                "retry",
            ],
            capabilities: &[
                "json-repair",
                "plugin-sampler",
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
            name: "loop",
            kind: "control-flow",
            required_fields: &["from", "max_iterations", "break_on", "body"],
            optional_fields: &["carry", "final", "on_iteration_error", "retain_iterations"],
            capabilities: &[
                "bounded-iteration",
                "body-subgraph",
                "explicit-break-condition",
                "loop-tracing",
            ],
        },
        StageMetadata {
            name: "map",
            kind: "control-flow",
            required_fields: &["from", "items_from", "max_items", "body"],
            optional_fields: &["final", "parallel", "max_concurrency"],
            capabilities: &["bounded-map", "body-subgraph", "json-array-items"],
        },
        StageMetadata {
            name: "tool",
            kind: "integration",
            required_fields: &["from", "command|url|transport"],
            optional_fields: &["method", "headers", "timeout_ms", "retry"],
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
        "rerank" => rerank(spec, input, cwd),
        "validate_json" => validate_json(spec, input, cwd),
        "extract" => extract(spec, input, cwd),
        "predicate" => predicate(spec, input, cwd),
        "score" => score(spec, input, cwd),
        "select" => select(spec, input, cwd),
        other => Err(LlmffError::UnknownStage(other.to_string())),
    }
}

pub(super) fn parse_json_stage_input(
    spec: &StageSpec,
    source: &str,
) -> Result<serde_json::Value, LlmffError> {
    serde_json::from_str(source).map_err(|error| LlmffError::StageExecution {
        stage_id: spec.id.clone(),
        message: format!("input is not valid JSON: {error}"),
    })
}

pub(super) fn render_messages_as_text(messages: &[Message]) -> String {
    messages
        .iter()
        .map(|message| format!("{}: {}", message.role, message.content))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn resolve_path(cwd: &Path, path: &str) -> std::path::PathBuf {
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

        let loop_stage = stages
            .iter()
            .find(|stage| stage.name == "loop")
            .expect("loop stage should be described");
        assert_eq!(loop_stage.kind, "control-flow");
        assert!(loop_stage.capabilities.contains(&"bounded-iteration"));
        assert!(loop_stage.capabilities.contains(&"loop-tracing"));

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
            agent: None,
            system: None,
            input: None,
            from: Some("load_prompt".to_string()),
            state_from: None,
            path: None,
            model: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            seed: None,
            response_format: None,
            stop: Vec::new(),
            sampler: None,
            schema: None,
            schema_path: None,
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            json_path: None,
            mode: None,
            criteria: None,
            score_field: None,
            reason_field: None,
            label_field: None,
            min_score: None,
            max_score: None,
            value: None,
            limit: None,
            dedupe_field: None,
            initial_carry: Default::default(),
            items_from: None,
            max_items: None,
            parallel: None,
            max_concurrency: None,
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
            index: None,
            timeout_ms: None,
            retry: None,
            cache_policy: None,
            max_iterations: None,
            break_on: None,
            carry: Default::default(),
            body: Vec::new(),
            final_output: None,
            on_iteration_error: None,
            retain_iterations: None,
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
            agent: None,
            system: None,
            input: None,
            from: Some("draft".to_string()),
            state_from: None,
            path: None,
            model: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            seed: None,
            response_format: None,
            stop: Vec::new(),
            sampler: None,
            schema: Some(r#"{"type":"object","required":["answer"]}"#.to_string()),
            schema_path: None,
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            json_path: None,
            mode: None,
            criteria: None,
            score_field: None,
            reason_field: None,
            label_field: None,
            min_score: None,
            max_score: None,
            value: None,
            limit: None,
            dedupe_field: None,
            initial_carry: Default::default(),
            items_from: None,
            max_items: None,
            parallel: None,
            max_concurrency: None,
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
            index: None,
            timeout_ms: None,
            retry: None,
            cache_policy: None,
            max_iterations: None,
            break_on: None,
            carry: Default::default(),
            body: Vec::new(),
            final_output: None,
            on_iteration_error: None,
            retain_iterations: None,
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
            agent: None,
            system: None,
            input: None,
            from: Some("draft".to_string()),
            state_from: None,
            path: None,
            model: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            seed: None,
            response_format: None,
            stop: Vec::new(),
            sampler: None,
            schema: None,
            schema_path: Some("answer.schema.json".to_string()),
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            json_path: None,
            mode: None,
            criteria: None,
            score_field: None,
            reason_field: None,
            label_field: None,
            min_score: None,
            max_score: None,
            value: None,
            limit: None,
            dedupe_field: None,
            initial_carry: Default::default(),
            items_from: None,
            max_items: None,
            parallel: None,
            max_concurrency: None,
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
            index: None,
            timeout_ms: None,
            retry: None,
            cache_policy: None,
            max_iterations: None,
            break_on: None,
            carry: Default::default(),
            body: Vec::new(),
            final_output: None,
            on_iteration_error: None,
            retain_iterations: None,
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
            agent: None,
            system: None,
            input: None,
            from: Some("draft".to_string()),
            state_from: None,
            path: None,
            model: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            seed: None,
            response_format: None,
            stop: Vec::new(),
            sampler: None,
            schema: None,
            schema_path: Some("missing.schema.json".to_string()),
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            json_path: None,
            mode: None,
            criteria: None,
            score_field: None,
            reason_field: None,
            label_field: None,
            min_score: None,
            max_score: None,
            value: None,
            limit: None,
            dedupe_field: None,
            initial_carry: Default::default(),
            items_from: None,
            max_items: None,
            parallel: None,
            max_concurrency: None,
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
            index: None,
            timeout_ms: None,
            retry: None,
            cache_policy: None,
            max_iterations: None,
            break_on: None,
            carry: Default::default(),
            body: Vec::new(),
            final_output: None,
            on_iteration_error: None,
            retain_iterations: None,
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
            agent: None,
            system: None,
            input: None,
            from: Some("load_prompt".to_string()),
            state_from: None,
            path: None,
            model: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            seed: None,
            response_format: None,
            stop: Vec::new(),
            sampler: None,
            schema: None,
            schema_path: None,
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            json_path: None,
            mode: None,
            criteria: None,
            score_field: None,
            reason_field: None,
            label_field: None,
            min_score: None,
            max_score: None,
            value: None,
            limit: None,
            dedupe_field: None,
            initial_carry: Default::default(),
            items_from: None,
            max_items: None,
            parallel: None,
            max_concurrency: None,
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
            index: None,
            timeout_ms: None,
            retry: None,
            cache_policy: None,
            max_iterations: None,
            break_on: None,
            carry: Default::default(),
            body: Vec::new(),
            final_output: None,
            on_iteration_error: None,
            retain_iterations: None,
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
            agent: None,
            system: None,
            input: None,
            from: Some("load_prompt".to_string()),
            state_from: None,
            path: None,
            model: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            seed: None,
            response_format: None,
            stop: Vec::new(),
            sampler: None,
            schema: None,
            schema_path: None,
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            json_path: None,
            mode: None,
            criteria: None,
            score_field: None,
            reason_field: None,
            label_field: None,
            min_score: None,
            max_score: None,
            value: None,
            limit: None,
            dedupe_field: None,
            initial_carry: Default::default(),
            items_from: None,
            max_items: None,
            parallel: None,
            max_concurrency: None,
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
            index: None,
            timeout_ms: None,
            retry: None,
            cache_policy: None,
            max_iterations: None,
            break_on: None,
            carry: Default::default(),
            body: Vec::new(),
            final_output: None,
            on_iteration_error: None,
            retain_iterations: None,
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
    fn retrieve_stage_persists_and_reuses_embedding_index() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("docs")).unwrap();
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
        let index_path = dir.path().join(".llmff/retrieve/context.index.json");
        let spec = StageSpec {
            id: "retrieve_context".to_string(),
            op: "retrieve".to_string(),
            agent: None,
            system: None,
            input: None,
            from: Some("load_prompt".to_string()),
            state_from: None,
            path: None,
            model: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            seed: None,
            response_format: None,
            stop: Vec::new(),
            sampler: None,
            schema: None,
            schema_path: None,
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            json_path: None,
            mode: None,
            criteria: None,
            score_field: None,
            reason_field: None,
            label_field: None,
            min_score: None,
            max_score: None,
            value: None,
            limit: None,
            dedupe_field: None,
            initial_carry: Default::default(),
            items_from: None,
            max_items: None,
            parallel: None,
            max_concurrency: None,
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
            index: Some(".llmff/retrieve/context.index.json".to_string()),
            timeout_ms: None,
            retry: None,
            cache_policy: None,
            max_iterations: None,
            break_on: None,
            carry: Default::default(),
            body: Vec::new(),
            final_output: None,
            on_iteration_error: None,
            retain_iterations: None,
        };

        let first =
            execute_deterministic_stage(&spec, Some(Value::Text("rust".to_string())), dir.path())
                .expect("first indexed retrieve should run");
        let StageStatus::Success(Value::Json(first_json)) = first else {
            panic!("retrieve should return JSON");
        };
        assert_eq!(
            first_json["index"]["path"],
            ".llmff/retrieve/context.index.json"
        );
        assert_eq!(first_json["index"]["reused_documents"], 0);
        assert_eq!(first_json["index"]["indexed_documents"], 2);
        assert!(index_path.exists());

        let second =
            execute_deterministic_stage(&spec, Some(Value::Text("rust".to_string())), dir.path())
                .expect("second indexed retrieve should run");
        let StageStatus::Success(Value::Json(second_json)) = second else {
            panic!("retrieve should return JSON");
        };
        assert_eq!(second_json["index"]["reused_documents"], 2);
        assert_eq!(second_json["index"]["indexed_documents"], 0);
        assert_eq!(second_json["matches"][0]["path"], "docs/trust.txt");

        std::fs::write(
            dir.path().join("docs/python.txt"),
            "Python tables can carry rust trust state.",
        )
        .unwrap();
        let third =
            execute_deterministic_stage(&spec, Some(Value::Text("rust".to_string())), dir.path())
                .expect("changed document should be re-indexed");
        let StageStatus::Success(Value::Json(third_json)) = third else {
            panic!("retrieve should return JSON");
        };
        assert_eq!(third_json["index"]["reused_documents"], 1);
        assert_eq!(third_json["index"]["indexed_documents"], 1);
    }

    #[test]
    fn rerank_stage_reorders_matches_with_embedding_strategy() {
        let spec = StageSpec {
            id: "rerank_context".to_string(),
            op: "rerank".to_string(),
            agent: None,
            system: None,
            input: None,
            from: Some("retrieve_context".to_string()),
            state_from: None,
            path: None,
            model: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            seed: None,
            response_format: None,
            stop: Vec::new(),
            sampler: None,
            schema: None,
            schema_path: None,
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            json_path: None,
            mode: None,
            criteria: None,
            score_field: None,
            reason_field: None,
            label_field: None,
            min_score: None,
            max_score: None,
            value: None,
            limit: None,
            dedupe_field: None,
            initial_carry: Default::default(),
            items_from: None,
            max_items: None,
            parallel: None,
            max_concurrency: None,
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
            index: None,
            timeout_ms: None,
            retry: None,
            cache_policy: None,
            max_iterations: None,
            break_on: None,
            carry: Default::default(),
            body: Vec::new(),
            final_output: None,
            on_iteration_error: None,
            retain_iterations: None,
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
            agent: None,
            system: None,
            input: None,
            from: Some("load_prompt".to_string()),
            state_from: None,
            path: Some("policy.md".to_string()),
            model: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            seed: None,
            response_format: None,
            stop: Vec::new(),
            sampler: None,
            schema: None,
            schema_path: None,
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            json_path: None,
            mode: None,
            criteria: None,
            score_field: None,
            reason_field: None,
            label_field: None,
            min_score: None,
            max_score: None,
            value: None,
            limit: None,
            dedupe_field: None,
            initial_carry: Default::default(),
            items_from: None,
            max_items: None,
            parallel: None,
            max_concurrency: None,
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
            index: None,
            timeout_ms: None,
            retry: None,
            cache_policy: None,
            max_iterations: None,
            break_on: None,
            carry: Default::default(),
            body: Vec::new(),
            final_output: None,
            on_iteration_error: None,
            retain_iterations: None,
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
            agent: None,
            system: None,
            input: None,
            from: Some("load_prompt".to_string()),
            state_from: None,
            path: Some("policy.md".to_string()),
            model: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            seed: None,
            response_format: None,
            stop: Vec::new(),
            sampler: None,
            schema: None,
            schema_path: None,
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            json_path: None,
            mode: None,
            criteria: None,
            score_field: None,
            reason_field: None,
            label_field: None,
            min_score: None,
            max_score: None,
            value: None,
            limit: None,
            dedupe_field: None,
            initial_carry: Default::default(),
            items_from: None,
            max_items: None,
            parallel: None,
            max_concurrency: None,
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
            index: None,
            timeout_ms: None,
            retry: None,
            cache_policy: None,
            max_iterations: None,
            break_on: None,
            carry: Default::default(),
            body: Vec::new(),
            final_output: None,
            on_iteration_error: None,
            retain_iterations: None,
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
            agent: None,
            system: None,
            input: None,
            from: Some("load_prompt".to_string()),
            state_from: None,
            path: Some("prompt.tmpl".to_string()),
            model: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            seed: None,
            response_format: None,
            stop: Vec::new(),
            sampler: None,
            schema: None,
            schema_path: None,
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            json_path: None,
            mode: None,
            criteria: None,
            score_field: None,
            reason_field: None,
            label_field: None,
            min_score: None,
            max_score: None,
            value: None,
            limit: None,
            dedupe_field: None,
            initial_carry: Default::default(),
            items_from: None,
            max_items: None,
            parallel: None,
            max_concurrency: None,
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
            index: None,
            timeout_ms: None,
            retry: None,
            cache_policy: None,
            max_iterations: None,
            break_on: None,
            carry: Default::default(),
            body: Vec::new(),
            final_output: None,
            on_iteration_error: None,
            retain_iterations: None,
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
            agent: None,
            system: None,
            input: None,
            from: Some("load_prompt".to_string()),
            state_from: None,
            path: Some("prompt.tmpl".to_string()),
            model: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            seed: None,
            response_format: None,
            stop: Vec::new(),
            sampler: None,
            schema: None,
            schema_path: None,
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            json_path: None,
            mode: None,
            criteria: None,
            score_field: None,
            reason_field: None,
            label_field: None,
            min_score: None,
            max_score: None,
            value: None,
            limit: None,
            dedupe_field: None,
            initial_carry: Default::default(),
            items_from: None,
            max_items: None,
            parallel: None,
            max_concurrency: None,
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
            index: None,
            timeout_ms: None,
            retry: None,
            cache_policy: None,
            max_iterations: None,
            break_on: None,
            carry: Default::default(),
            body: Vec::new(),
            final_output: None,
            on_iteration_error: None,
            retain_iterations: None,
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
            agent: None,
            system: None,
            input: None,
            from: Some("load_prompt".to_string()),
            state_from: None,
            path: Some("prompt.tmpl".to_string()),
            model: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            seed: None,
            response_format: None,
            stop: Vec::new(),
            sampler: None,
            schema: None,
            schema_path: None,
            when: None,
            on_success: None,
            on_invalid: None,
            on_skipped: None,
            field: None,
            json_path: None,
            mode: None,
            criteria: None,
            score_field: None,
            reason_field: None,
            label_field: None,
            min_score: None,
            max_score: None,
            value: None,
            limit: None,
            dedupe_field: None,
            initial_carry: Default::default(),
            items_from: None,
            max_items: None,
            parallel: None,
            max_concurrency: None,
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
            index: None,
            timeout_ms: None,
            retry: None,
            cache_policy: None,
            max_iterations: None,
            break_on: None,
            carry: Default::default(),
            body: Vec::new(),
            final_output: None,
            on_iteration_error: None,
            retain_iterations: None,
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
