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
    #[error("stage `{stage_id}` failed: stage timed out")]
    StageTimeout { stage_id: String },
    #[error("stage `{stage_id}` failed: {kind}")]
    HttpTool {
        stage_id: String,
        kind: HttpToolFailure,
    },
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

/// Sub-kinds of `LlmffError::HttpTool`, typed so retry eligibility and
/// classification never need to re-derive meaning from message text.
#[derive(Debug, Clone, Error)]
pub enum HttpToolFailure {
    #[error("http tool requires method")]
    RequiresMethod,
    #[error("http tool requires url")]
    RequiresUrl,
    #[error("http tool request failed: {0}")]
    RequestFailed(String),
    #[error("http tool returned status {status_text}: {body}")]
    Status {
        status_code: u16,
        status_text: String,
        body: String,
    },
}

impl HttpToolFailure {
    pub fn is_retryable(&self) -> bool {
        match self {
            HttpToolFailure::RequestFailed(_) => true,
            HttpToolFailure::Status { status_code, .. } => {
                matches!(status_code, 500 | 502 | 503 | 504)
            }
            HttpToolFailure::RequiresMethod | HttpToolFailure::RequiresUrl => false,
        }
    }
}

/// The failure-kind registry in `docs/schemas/failure-kinds-v1.json`. Variant
/// names here are the classification source of truth; `as_str` values are
/// the frozen wire strings and must not change without updating the schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    ManifestParse,
    Io,
    Json,
    GraphValidation,
    UnknownStage,
    Timeout,
    Http,
    StageExecution,
    Backend,
    Config,
    NotImplemented,
}

impl FailureKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            FailureKind::ManifestParse => "manifest_parse",
            FailureKind::Io => "io",
            FailureKind::Json => "json",
            FailureKind::GraphValidation => "graph_validation",
            FailureKind::UnknownStage => "unknown_stage",
            FailureKind::Timeout => "timeout",
            FailureKind::Http => "http",
            FailureKind::StageExecution => "stage_execution",
            FailureKind::Backend => "backend",
            FailureKind::Config => "config",
            FailureKind::NotImplemented => "not_implemented",
        }
    }

    pub fn default_message(&self) -> &'static str {
        match self {
            FailureKind::ManifestParse => "manifest parse failed",
            FailureKind::Io => "I/O operation failed",
            FailureKind::Json => "JSON operation failed",
            FailureKind::GraphValidation => "graph validation failed",
            FailureKind::UnknownStage => "unknown stage operation",
            FailureKind::Timeout => "stage timed out",
            FailureKind::Http => "HTTP stage failed",
            FailureKind::StageExecution => "stage execution failed",
            FailureKind::Backend => "backend request failed",
            FailureKind::Config => "configuration failed",
            FailureKind::NotImplemented => "feature is not implemented",
        }
    }
}

impl std::fmt::Display for FailureKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl LlmffError {
    pub fn failure_kind(&self) -> FailureKind {
        match self {
            LlmffError::ManifestParse(_) => FailureKind::ManifestParse,
            LlmffError::Io(_) => FailureKind::Io,
            LlmffError::Json(_) => FailureKind::Json,
            LlmffError::GraphValidation(_) => FailureKind::GraphValidation,
            LlmffError::UnknownStage(_) => FailureKind::UnknownStage,
            LlmffError::StageTimeout { .. } => FailureKind::Timeout,
            LlmffError::HttpTool { .. } => FailureKind::Http,
            LlmffError::StageExecution { .. } => FailureKind::StageExecution,
            LlmffError::LoopStageExecution { source, .. } => source.failure_kind(),
            LlmffError::Backend(_) => FailureKind::Backend,
            LlmffError::Config(_) => FailureKind::Config,
            LlmffError::NotImplemented(_) => FailureKind::NotImplemented,
        }
    }

    pub fn failure_message(&self) -> &'static str {
        self.failure_kind().default_message()
    }
}
