use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

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

#[derive(Debug, Clone)]
pub struct OpenAiCompatibleBackend {
    base_url: String,
    api_key: String,
    client: reqwest::Client,
}

impl OpenAiCompatibleBackend {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl Backend for OpenAiCompatibleBackend {
    async fn infer(&self, request: InferRequest) -> Result<InferResponse, LlmffError> {
        let url = format!("{}/v1/chat/completions", self.base_url);
        let body = json!({
            "model": request.model,
            "messages": [
                {
                    "role": "user",
                    "content": request.prompt
                }
            ],
            "temperature": request.temperature,
        });
        let mut http_request = self.client.post(url).json(&body);
        if !self.api_key.is_empty() {
            http_request = http_request.bearer_auth(&self.api_key);
        }

        let response = http_request
            .send()
            .await
            .map_err(|error| LlmffError::Backend(format!("request failed: {error}")))?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(LlmffError::Backend(format!(
                "OpenAI-compatible backend returned {status}: {body}"
            )));
        }

        let completion: ChatCompletionResponse = response
            .json()
            .await
            .map_err(|error| LlmffError::Backend(format!("invalid response JSON: {error}")))?;
        let text = completion
            .choices
            .first()
            .and_then(|choice| choice.message.content.as_deref())
            .ok_or_else(|| {
                LlmffError::Backend("OpenAI-compatible response missing choice content".to_string())
            })?
            .to_string();

        Ok(InferResponse {
            model: request.model,
            text,
        })
    }
}

#[derive(Debug, Clone)]
pub struct OllamaBackend {
    base_url: String,
    client: reqwest::Client,
}

impl OllamaBackend {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl Backend for OllamaBackend {
    async fn infer(&self, request: InferRequest) -> Result<InferResponse, LlmffError> {
        let url = format!("{}/api/chat", self.base_url);
        let body = ollama_chat_request_body(&request);
        let response = self
            .client
            .post(url)
            .json(&body)
            .send()
            .await
            .map_err(|error| LlmffError::Backend(format!("request failed: {error}")))?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(LlmffError::Backend(format!(
                "Ollama backend returned {status}: {body}"
            )));
        }

        let completion: OllamaChatResponse = response
            .json()
            .await
            .map_err(|error| LlmffError::Backend(format!("invalid response JSON: {error}")))?;
        let text = completion.message.content.ok_or_else(|| {
            LlmffError::Backend("Ollama response missing message content".to_string())
        })?;

        Ok(InferResponse {
            model: request.model,
            text,
        })
    }
}

fn ollama_chat_request_body(request: &InferRequest) -> serde_json::Value {
    let mut body = json!({
        "model": request.model,
        "messages": [
            {
                "role": "user",
                "content": request.prompt
            }
        ],
        "stream": false,
    });

    if let Some(temperature) = request.temperature {
        body["options"] = json!({ "temperature": temperature });
    }

    body
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    message: OllamaMessage,
}

#[derive(Debug, Deserialize)]
struct OllamaMessage {
    content: Option<String>,
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

    #[tokio::test]
    async fn openai_compatible_backend_reads_chat_completion_content() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [
                    {
                        "message": {
                            "content": "hello from backend"
                        }
                    }
                ]
            })))
            .mount(&server)
            .await;

        let backend = OpenAiCompatibleBackend::new(server.uri(), "");
        let response = backend
            .infer(InferRequest {
                model: "test-model".to_string(),
                prompt: "Say hello".to_string(),
                temperature: Some(0.0),
            })
            .await
            .unwrap();

        assert_eq!(response.model, "test-model");
        assert_eq!(response.text, "hello from backend");
    }

    #[tokio::test]
    async fn ollama_backend_reads_chat_message_content() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "model": "llama3.1",
                "message": {
                    "role": "assistant",
                    "content": "hello from ollama"
                },
                "done": true
            })))
            .mount(&server)
            .await;

        let backend = OllamaBackend::new(server.uri());
        let response = backend
            .infer(InferRequest {
                model: "llama3.1".to_string(),
                prompt: "Say hello".to_string(),
                temperature: Some(0.2),
            })
            .await
            .unwrap();

        assert_eq!(response.model, "llama3.1");
        assert_eq!(response.text, "hello from ollama");

        let requests = server.received_requests().await.unwrap();
        let body: serde_json::Value = requests[0].body_json().unwrap();
        assert_eq!(body["model"], "llama3.1");
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"], "Say hello");
        assert_eq!(body["stream"], false);
        assert!(
            (body["options"]["temperature"].as_f64().unwrap() - 0.2).abs() < 0.000_001
        );
    }
}
