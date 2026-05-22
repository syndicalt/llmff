use async_trait::async_trait;

use crate::error::LlmffError;

#[derive(Debug, Clone, PartialEq)]
pub struct InferRequest {
    pub model: String,
    pub prompt: String,
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InferResponse {
    pub model: String,
    pub text: String,
}

#[async_trait]
pub trait Backend: Send + Sync {
    async fn infer(&self, request: InferRequest) -> Result<InferResponse, LlmffError>;
}

#[derive(Debug, Clone)]
pub struct MockBackend {
    model: String,
    response: String,
}

impl MockBackend {
    pub fn new(model: impl Into<String>, response: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            response: response.into(),
        }
    }
}

#[async_trait]
impl Backend for MockBackend {
    async fn infer(&self, request: InferRequest) -> Result<InferResponse, LlmffError> {
        if request.model != self.model {
            return Err(LlmffError::Backend(format!(
                "mock backend does not serve model `{}`",
                request.model
            )));
        }

        Ok(InferResponse {
            model: request.model,
            text: self.response.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_backend_returns_configured_response() {
        let backend = MockBackend::new("mock:json", r#"{"answer":"ok"}"#);
        let response = backend
            .infer(InferRequest {
                model: "mock:json".to_string(),
                prompt: "Return JSON".to_string(),
                temperature: Some(0.2),
            })
            .await
            .expect("mock backend should respond");

        assert_eq!(response.text, r#"{"answer":"ok"}"#);
        assert_eq!(response.model, "mock:json");
    }
}
