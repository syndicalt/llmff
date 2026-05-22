use thiserror::Error;

#[derive(Debug, Error)]
pub enum LlmffError {
    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
}
