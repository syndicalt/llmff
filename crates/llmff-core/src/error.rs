use thiserror::Error;

#[derive(Debug, Error)]
pub enum LlmffError {
    #[error("failed to parse manifest: {0}")]
    ManifestParse(#[from] serde_yaml::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("graph validation failed: {0}")]
    GraphValidation(String),
    #[error("unknown stage operation `{0}`")]
    UnknownStage(String),
    #[error("stage `{stage_id}` failed: {message}")]
    StageExecution { stage_id: String, message: String },
    #[error("loop body stage `{stage_id}` failed: {source}")]
    LoopStageExecution {
        stage_id: String,
        loop_id: String,
        loop_iteration: usize,
        loop_stage_id: String,
        #[source]
        source: Box<LlmffError>,
    },
    #[error("backend error: {0}")]
    Backend(String),
    #[error("configuration error: {0}")]
    Config(String),
    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
}
