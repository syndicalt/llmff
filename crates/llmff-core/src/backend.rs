use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

use crate::error::LlmffError;

#[derive(Debug, Clone, PartialEq)]
pub struct InferRequest {
    pub model: String,
    pub prompt: String,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InferResponse {
    pub model: String,
    pub text: String,
    pub usage: Option<UsageMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageMetadata {
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
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
            usage: None,
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
        let mut body = json!({
            "model": request.model,
            "messages": [
                {
                    "role": "user",
                    "content": request.prompt
                }
            ],
        });
        if let Some(temperature) = request.temperature {
            body["temperature"] = json!(temperature);
        }
        if let Some(top_p) = request.top_p {
            body["top_p"] = json!(top_p);
        }
        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = json!(max_tokens);
        }
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
            usage: completion.usage.map(Into::into),
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
        let usage = ollama_usage(&completion);
        let text = completion.message.content.ok_or_else(|| {
            LlmffError::Backend("Ollama response missing message content".to_string())
        })?;

        Ok(InferResponse {
            model: request.model,
            text,
            usage,
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

    let mut options = serde_json::Map::new();
    if let Some(temperature) = request.temperature {
        options.insert("temperature".to_string(), json!(temperature));
    }
    if let Some(top_p) = request.top_p {
        options.insert("top_p".to_string(), json!(top_p));
    }
    if let Some(max_tokens) = request.max_tokens {
        options.insert("num_predict".to_string(), json!(max_tokens));
    }
    if !options.is_empty() {
        body["options"] = serde_json::Value::Object(options);
    }

    body
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
    usage: Option<OpenAiUsage>,
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
struct OpenAiUsage {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
    total_tokens: Option<u64>,
}

impl From<OpenAiUsage> for UsageMetadata {
    fn from(usage: OpenAiUsage) -> Self {
        Self {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
        }
    }
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    message: OllamaMessage,
    prompt_eval_count: Option<u64>,
    eval_count: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct OllamaMessage {
    content: Option<String>,
}

fn ollama_usage(response: &OllamaChatResponse) -> Option<UsageMetadata> {
    if response.prompt_eval_count.is_none() && response.eval_count.is_none() {
        return None;
    }

    Some(UsageMetadata {
        prompt_tokens: response.prompt_eval_count,
        completion_tokens: response.eval_count,
        total_tokens: match (response.prompt_eval_count, response.eval_count) {
            (Some(prompt), Some(completion)) => Some(prompt + completion),
            _ => None,
        },
    })
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
                top_p: None,
                max_tokens: None,
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
                ],
                "usage": {
                    "prompt_tokens": 12,
                    "completion_tokens": 8,
                    "total_tokens": 20
                }
            })))
            .mount(&server)
            .await;

        let backend = OpenAiCompatibleBackend::new(server.uri(), "");
        let response = backend
            .infer(InferRequest {
                model: "test-model".to_string(),
                prompt: "Say hello".to_string(),
                temperature: Some(0.0),
                top_p: Some(0.9),
                max_tokens: Some(256),
            })
            .await
            .unwrap();

        assert_eq!(response.model, "test-model");
        assert_eq!(response.text, "hello from backend");
        let usage = response.usage.expect("usage should be parsed");
        assert_eq!(usage.prompt_tokens, Some(12));
        assert_eq!(usage.completion_tokens, Some(8));
        assert_eq!(usage.total_tokens, Some(20));

        let requests = server.received_requests().await.unwrap();
        let body: serde_json::Value = requests[0].body_json().unwrap();
        assert!((body["top_p"].as_f64().unwrap() - 0.9).abs() < 0.000_001);
        assert_eq!(body["max_tokens"], 256);
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
                "done": true,
                "prompt_eval_count": 7,
                "eval_count": 5
            })))
            .mount(&server)
            .await;

        let backend = OllamaBackend::new(server.uri());
        let response = backend
            .infer(InferRequest {
                model: "llama3.1".to_string(),
                prompt: "Say hello".to_string(),
                temperature: Some(0.2),
                top_p: Some(0.8),
                max_tokens: Some(128),
            })
            .await
            .unwrap();

        assert_eq!(response.model, "llama3.1");
        assert_eq!(response.text, "hello from ollama");
        let usage = response.usage.expect("usage should be parsed");
        assert_eq!(usage.prompt_tokens, Some(7));
        assert_eq!(usage.completion_tokens, Some(5));
        assert_eq!(usage.total_tokens, Some(12));

        let requests = server.received_requests().await.unwrap();
        let body: serde_json::Value = requests[0].body_json().unwrap();
        assert_eq!(body["model"], "llama3.1");
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"], "Say hello");
        assert_eq!(body["stream"], false);
        assert!((body["options"]["temperature"].as_f64().unwrap() - 0.2).abs() < 0.000_001);
        assert!((body["options"]["top_p"].as_f64().unwrap() - 0.8).abs() < 0.000_001);
        assert_eq!(body["options"]["num_predict"], 128);
    }
}
