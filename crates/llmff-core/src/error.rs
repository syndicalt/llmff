use thiserror::Error;

#[derive(Debug, Error)]
pub enum LlmffError {
    #[error("failed to parse manifest: {0}")]
    ManifestParse(#[from] serde_yaml::Error),
    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
}
