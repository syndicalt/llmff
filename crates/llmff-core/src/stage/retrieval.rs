use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::LlmffError;
use crate::manifest::StageSpec;
use crate::value::{StageStatus, Value};

use super::specs::{RerankSpec, RetrieveSpec, RetrieveStrategy};
use super::{render_messages_as_text, resolve_path};

pub(super) fn retrieve(
    spec: &StageSpec,
    input: Option<Value>,
    cwd: &Path,
) -> Result<StageStatus, LlmffError> {
    let query = input
        .map(render_value_as_text)
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: "retrieve requires input".to_string(),
        })?;
    let typed = RetrieveSpec::parse(spec).map_err(|message| LlmffError::StageExecution {
        stage_id: spec.id.clone(),
        message,
    })?;
    if typed.strategy == RetrieveStrategy::Command {
        return command_retrieve(spec, &typed, cwd, query);
    }
    let indexed_documents = if typed.strategy == RetrieveStrategy::Embedding {
        load_retrieve_documents(spec, &typed, cwd)?
    } else {
        RetrieveDocuments::unindexed(read_retrieve_documents(spec, &typed.documents, cwd)?)
    };
    let query_terms = tokenize(&query);
    let query_embedding = match typed.strategy {
        RetrieveStrategy::Lexical => BTreeMap::new(),
        RetrieveStrategy::Embedding => hashed_embedding(&query),
        RetrieveStrategy::Command => BTreeMap::new(),
    };
    let mut matches = Vec::new();

    for document in &indexed_documents.documents {
        let score = score_document(typed.strategy, &query_terms, &query_embedding, document);
        if score.rank > 0.0 {
            matches.push(RetrieveMatch {
                path: document.path.clone(),
                score,
                text: document.text.clone(),
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
    if let Some(top_k) = typed.top_k {
        matches.truncate(top_k);
    }

    let mut output = serde_json::json!({
        "query": query,
        "strategy": typed.strategy.as_str(),
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
    });
    if let Some(index) = indexed_documents.index {
        output["index"] = serde_json::json!({
            "path": index.path,
            "reused_documents": index.reused_documents,
            "indexed_documents": index.indexed_documents,
        });
    }

    Ok(StageStatus::Success(Value::Json(output)))
}

pub(super) fn rerank(
    spec: &StageSpec,
    input: Option<Value>,
    cwd: &Path,
) -> Result<StageStatus, LlmffError> {
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
    let typed = RerankSpec::parse(spec).map_err(|message| LlmffError::StageExecution {
        stage_id: spec.id.clone(),
        message,
    })?;
    if typed.strategy == RetrieveStrategy::Command {
        if let Some(top_k) = typed.top_k {
            root.insert(
                "top_k".to_string(),
                serde_json::Value::Number(serde_json::Number::from(top_k)),
            );
        }
        let command = typed
            .command
            .as_deref()
            .expect("RerankSpec::parse guarantees command for command strategy");
        return execute_json_command(
            spec,
            cwd,
            serde_json::Value::Object(root),
            "rerank command",
            command,
        );
    }
    let query_terms = tokenize(&query);
    let query_embedding = match typed.strategy {
        RetrieveStrategy::Lexical => BTreeMap::new(),
        RetrieveStrategy::Embedding => hashed_embedding(&query),
        RetrieveStrategy::Command => BTreeMap::new(),
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
        let score = score_text(typed.strategy, &query_terms, &query_embedding, &text);
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
    if let Some(top_k) = typed.top_k {
        reranked.truncate(top_k);
    }

    root.insert("query".to_string(), serde_json::Value::String(query));
    root.insert(
        "strategy".to_string(),
        serde_json::Value::String(typed.strategy.as_str().to_string()),
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

fn command_retrieve(
    spec: &StageSpec,
    typed: &RetrieveSpec,
    cwd: &Path,
    query: String,
) -> Result<StageStatus, LlmffError> {
    let mut documents = Vec::new();
    for document in &typed.documents {
        let text = std::fs::read_to_string(resolve_path(cwd, document)).map_err(|error| {
            LlmffError::StageExecution {
                stage_id: spec.id.clone(),
                message: format!("failed to read retrieve document `{document}`: {error}"),
            }
        })?;
        documents.push(serde_json::json!({
            "path": document,
            "text": text,
        }));
    }

    let mut request = serde_json::Map::new();
    request.insert("query".to_string(), serde_json::Value::String(query));
    request.insert("documents".to_string(), serde_json::Value::Array(documents));
    if let Some(top_k) = typed.top_k {
        request.insert(
            "top_k".to_string(),
            serde_json::Value::Number(serde_json::Number::from(top_k)),
        );
    }

    let command = typed
        .command
        .as_deref()
        .expect("RetrieveSpec::parse guarantees command for command strategy");
    execute_json_command(
        spec,
        cwd,
        serde_json::Value::Object(request),
        "retrieve command",
        command,
    )
}

fn load_retrieve_documents(
    spec: &StageSpec,
    typed: &RetrieveSpec,
    cwd: &Path,
) -> Result<RetrieveDocuments, LlmffError> {
    let Some(index_path) = typed.index.as_deref() else {
        return Ok(RetrieveDocuments::unindexed(read_retrieve_documents(
            spec,
            &typed.documents,
            cwd,
        )?));
    };
    let resolved_index_path = resolve_path(cwd, index_path);
    let existing_index = read_retrieve_index(spec, &resolved_index_path)?;
    let mut existing_entries = existing_index
        .records
        .into_iter()
        .map(|entry| (entry.path.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    let mut records = Vec::new();
    let mut documents = Vec::new();
    let mut reused_documents = 0usize;
    let mut indexed_documents = 0usize;

    for document in &typed.documents {
        let metadata = retrieve_document_metadata(spec, cwd, document)?;
        let existing = existing_entries.remove(document);
        let record = if let Some(record) = existing {
            if record.metadata == metadata {
                reused_documents += 1;
                record
            } else {
                indexed_documents += 1;
                build_retrieve_index_record(spec, cwd, document, metadata)?
            }
        } else {
            indexed_documents += 1;
            build_retrieve_index_record(spec, cwd, document, metadata)?
        };
        documents.push(IndexedRetrieveDocument {
            path: record.path.clone(),
            text: record.text.clone(),
            embedding: record.embedding.clone(),
        });
        records.push(record);
    }

    write_retrieve_index(
        spec,
        &resolved_index_path,
        &RetrieveIndexRecord {
            version: RETRIEVE_INDEX_VERSION,
            strategy: RetrieveStrategy::Embedding.as_str().to_string(),
            records,
        },
    )?;

    Ok(RetrieveDocuments {
        documents,
        index: Some(RetrieveIndexUsage {
            path: index_path.to_string(),
            reused_documents,
            indexed_documents,
        }),
    })
}

fn read_retrieve_documents(
    spec: &StageSpec,
    documents: &[String],
    cwd: &Path,
) -> Result<Vec<IndexedRetrieveDocument>, LlmffError> {
    documents
        .iter()
        .map(|document| {
            let text = read_retrieve_document_text(spec, cwd, document)?;
            Ok(IndexedRetrieveDocument {
                path: document.clone(),
                embedding: hashed_embedding(&text),
                text,
            })
        })
        .collect()
}

fn read_retrieve_index(
    spec: &StageSpec,
    index_path: &Path,
) -> Result<RetrieveIndexRecord, LlmffError> {
    if !index_path.exists() {
        return Ok(RetrieveIndexRecord {
            version: RETRIEVE_INDEX_VERSION,
            strategy: RetrieveStrategy::Embedding.as_str().to_string(),
            records: Vec::new(),
        });
    }

    let source =
        std::fs::read_to_string(index_path).map_err(|error| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: format!(
                "failed to read retrieve index `{}`: {error}",
                index_path.display()
            ),
        })?;
    let index: RetrieveIndexRecord =
        serde_json::from_str(&source).map_err(|error| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: format!("invalid retrieve index `{}`: {error}", index_path.display()),
        })?;
    if index.version != RETRIEVE_INDEX_VERSION {
        return Err(LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: format!(
                "unsupported retrieve index `{}` version {}",
                index_path.display(),
                index.version
            ),
        });
    }
    if index.strategy != RetrieveStrategy::Embedding.as_str() {
        return Err(LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: format!(
                "retrieve index `{}` strategy must be embedding",
                index_path.display()
            ),
        });
    }

    Ok(index)
}

fn build_retrieve_index_record(
    spec: &StageSpec,
    cwd: &Path,
    document: &str,
    metadata: RetrieveDocumentMetadata,
) -> Result<RetrieveIndexEntry, LlmffError> {
    let text = read_retrieve_document_text(spec, cwd, document)?;
    Ok(RetrieveIndexEntry {
        path: document.to_string(),
        metadata,
        embedding: hashed_embedding(&text),
        text,
    })
}

fn retrieve_document_metadata(
    spec: &StageSpec,
    cwd: &Path,
    document: &str,
) -> Result<RetrieveDocumentMetadata, LlmffError> {
    let path = resolve_path(cwd, document);
    let metadata = std::fs::metadata(&path).map_err(|error| LlmffError::StageExecution {
        stage_id: spec.id.clone(),
        message: format!("failed to stat retrieve document `{document}`: {error}"),
    })?;
    let modified = metadata
        .modified()
        .map_err(|error| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: format!("failed to read retrieve document `{document}` mtime: {error}"),
        })?
        .duration_since(UNIX_EPOCH)
        .map_err(|error| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: format!("invalid retrieve document `{document}` mtime: {error}"),
        })?;

    Ok(RetrieveDocumentMetadata {
        len: metadata.len(),
        modified_unix_nanos: modified.as_nanos(),
    })
}

fn read_retrieve_document_text(
    spec: &StageSpec,
    cwd: &Path,
    document: &str,
) -> Result<String, LlmffError> {
    std::fs::read_to_string(resolve_path(cwd, document)).map_err(|error| {
        LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: format!("failed to read retrieve document `{document}`: {error}"),
        }
    })
}

fn write_retrieve_index(
    spec: &StageSpec,
    index_path: &Path,
    index: &RetrieveIndexRecord,
) -> Result<(), LlmffError> {
    let parent = index_path
        .parent()
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: format!(
                "retrieve index `{}` has no parent directory",
                index_path.display()
            ),
        })?;
    if !parent.as_os_str().is_empty() {
        std::fs::create_dir_all(parent).map_err(|error| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: format!(
                "failed to create retrieve index directory `{}`: {error}",
                parent.display()
            ),
        })?;
    }
    let encoded = serde_json::to_vec_pretty(index).map_err(LlmffError::Json)?;
    let tmp_path = index_path.with_extension(format!(
        "tmp-{}",
        retrieve_index_write_digest(index_path, &encoded)
    ));
    std::fs::write(&tmp_path, encoded).map_err(|error| LlmffError::StageExecution {
        stage_id: spec.id.clone(),
        message: format!(
            "failed to write retrieve index `{}`: {error}",
            tmp_path.display()
        ),
    })?;
    std::fs::rename(&tmp_path, index_path).map_err(|error| LlmffError::StageExecution {
        stage_id: spec.id.clone(),
        message: format!(
            "failed to move retrieve index `{}` into `{}`: {error}",
            tmp_path.display(),
            index_path.display()
        ),
    })
}

fn retrieve_index_write_digest(index_path: &Path, encoded: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(index_path.as_os_str().as_encoded_bytes());
    hasher.update(encoded);
    format!("{:x}", hasher.finalize())
}

fn execute_json_command(
    spec: &StageSpec,
    cwd: &Path,
    request: serde_json::Value,
    label: &str,
    command: &[String],
) -> Result<StageStatus, LlmffError> {
    let program = command.first().ok_or_else(|| LlmffError::StageExecution {
        stage_id: spec.id.clone(),
        message: format!("{label} cannot be empty"),
    })?;
    let encoded = serde_json::to_vec(&request).map_err(LlmffError::Json)?;
    let mut child = Command::new(resolve_command_path(cwd, program))
        .args(&command[1..])
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: format!("failed to start {label} `{program}`: {error}"),
        })?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: format!("failed to open {label} stdin"),
        })?;
    stdin
        .write_all(&encoded)
        .map_err(|error| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: format!("failed to write {label} stdin: {error}"),
        })?;
    drop(stdin);

    let output = child
        .wait_with_output()
        .map_err(|error| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: format!("failed to wait for {label} `{program}`: {error}"),
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: format!(
                "{label} exited with status {}: {}",
                output.status,
                stderr.trim()
            ),
        });
    }

    let response: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|error| LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: format!("{label} returned invalid JSON: {error}"),
        })?;
    if !response.is_object() {
        return Err(LlmffError::StageExecution {
            stage_id: spec.id.clone(),
            message: format!("{label} must return a JSON object"),
        });
    }
    Ok(StageStatus::Success(Value::Json(response)))
}

const RETRIEVE_INDEX_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
struct RetrieveIndexRecord {
    version: u32,
    strategy: String,
    records: Vec<RetrieveIndexEntry>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
struct RetrieveIndexEntry {
    path: String,
    metadata: RetrieveDocumentMetadata,
    text: String,
    embedding: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
struct RetrieveDocumentMetadata {
    len: u64,
    modified_unix_nanos: u128,
}

struct RetrieveDocuments {
    documents: Vec<IndexedRetrieveDocument>,
    index: Option<RetrieveIndexUsage>,
}

impl RetrieveDocuments {
    fn unindexed(documents: Vec<IndexedRetrieveDocument>) -> Self {
        Self {
            documents,
            index: None,
        }
    }
}

struct RetrieveIndexUsage {
    path: String,
    reused_documents: usize,
    indexed_documents: usize,
}

struct IndexedRetrieveDocument {
    path: String,
    text: String,
    embedding: BTreeMap<String, f64>,
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
        RetrieveStrategy::Command => unreachable!("command strategy is executed externally"),
    }
}

fn score_document(
    strategy: RetrieveStrategy,
    query_terms: &BTreeSet<String>,
    query_embedding: &BTreeMap<String, f64>,
    document: &IndexedRetrieveDocument,
) -> RetrieveScore {
    match strategy {
        RetrieveStrategy::Lexical => {
            score_text(strategy, query_terms, query_embedding, &document.text)
        }
        RetrieveStrategy::Embedding => cosine_similarity(query_embedding, &document.embedding),
        RetrieveStrategy::Command => unreachable!("command strategy is executed externally"),
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

fn resolve_command_path(cwd: &Path, program: &str) -> std::path::PathBuf {
    let path = Path::new(program);
    if path.is_relative() && path.components().count() > 1 {
        cwd.join(path)
    } else {
        path.to_path_buf()
    }
}
