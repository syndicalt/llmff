use async_trait::async_trait;
use futures::{stream, Stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::{Command, Stdio};

use crate::error::LlmffError;
use crate::value::Message;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InferRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_tokens: Option<u32>,
    pub seed: Option<u64>,
    pub response_format: Option<String>,
    pub stop: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InferResponse {
    pub model: String,
    pub text: String,
    pub usage: Option<UsageMetadata>,
}

pub type InferStream = Pin<Box<dyn Stream<Item = Result<InferStreamChunk, LlmffError>> + Send>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InferStreamChunk {
    pub delta: String,
    pub done: bool,
    pub usage: Option<UsageMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct UsageMetadata {
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct CommandBackend {
    alias: String,
    entrypoint: PathBuf,
}

impl CommandBackend {
    pub fn new(alias: impl Into<String>, entrypoint: impl Into<PathBuf>) -> Self {
        Self {
            alias: alias.into(),
            entrypoint: entrypoint.into(),
        }
    }
}

#[async_trait]
impl Backend for CommandBackend {
    async fn infer(&self, request: InferRequest) -> Result<InferResponse, LlmffError> {
        let encoded = serde_json::to_vec(&request).map_err(LlmffError::Json)?;
        let mut child = Command::new(resolve_command_path(&self.entrypoint))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                LlmffError::Backend(format!(
                    "failed to start plugin backend `{}`: {error}",
                    self.alias
                ))
            })?;
        let mut stdin = child.stdin.take().ok_or_else(|| {
            LlmffError::Backend(format!(
                "failed to open plugin backend `{}` stdin",
                self.alias
            ))
        })?;
        stdin.write_all(&encoded).map_err(|error| {
            LlmffError::Backend(format!(
                "failed to write plugin backend `{}` request: {error}",
                self.alias
            ))
        })?;
        drop(stdin);

        let output = child.wait_with_output().map_err(|error| {
            LlmffError::Backend(format!(
                "failed to wait for plugin backend `{}`: {error}",
                self.alias
            ))
        })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(LlmffError::Backend(format!(
                "plugin backend `{}` exited with status {}: {}",
                self.alias,
                output.status,
                stderr.trim()
            )));
        }

        let response: CommandBackendResponse =
            serde_json::from_slice(&output.stdout).map_err(|error| {
                LlmffError::Backend(format!(
                    "plugin backend `{}` returned invalid response JSON: {error}",
                    self.alias
                ))
            })?;

        Ok(InferResponse {
            model: request.model,
            text: response.text,
            usage: response.usage,
        })
    }
}

#[derive(Debug, Deserialize)]
struct CommandBackendResponse {
    text: String,
    usage: Option<UsageMetadata>,
}

fn resolve_command_path(path: &Path) -> PathBuf {
    path.to_path_buf()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BackendFamily {
    pub name: &'static str,
    pub kind: &'static str,
    pub registration_flag: &'static str,
    pub requires_api_key: bool,
    pub model_aliases: &'static [&'static str],
    pub capabilities: &'static [&'static str],
}

pub fn builtin_backend_families() -> &'static [BackendFamily] {
    &[
        BackendFamily {
            name: "mock",
            kind: "deterministic",
            registration_flag: "built-in",
            requires_api_key: false,
            model_aliases: &["mock:bad", "mock:good", "mock:json"],
            capabilities: &["deterministic", "chat-messages"],
        },
        BackendFamily {
            name: "ollama",
            kind: "local-chat",
            registration_flag: "--ollama <alias>=<base-url>",
            requires_api_key: false,
            model_aliases: &[],
            capabilities: &[
                "chat-messages",
                "response-format-json",
                "sampling",
                "seed-control",
                "stop-sequences",
                "usage-metadata",
            ],
        },
        BackendFamily {
            name: "openai-compatible",
            kind: "remote-chat",
            registration_flag: "--backend <alias>=<base-url>",
            requires_api_key: true,
            model_aliases: &[],
            capabilities: &[
                "chat-messages",
                "response-format-json",
                "sampling",
                "seed-control",
                "streaming-inference",
                "stop-sequences",
                "usage-metadata",
            ],
        },
    ]
}

#[async_trait]
pub trait Backend: Send + Sync {
    async fn infer(&self, request: InferRequest) -> Result<InferResponse, LlmffError>;

    fn stream(&self, _request: InferRequest) -> InferStream {
        Box::pin(stream::once(async {
            Err(LlmffError::Backend(
                "streaming inference is not supported by this backend".to_string(),
            ))
        }))
    }
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

    fn stream(&self, request: InferRequest) -> InferStream {
        let result = if request.model != self.model {
            Err(LlmffError::Backend(format!(
                "mock backend does not serve model `{}`",
                request.model
            )))
        } else {
            Ok(InferStreamChunk {
                delta: self.response.clone(),
                done: true,
                usage: None,
            })
        };

        Box::pin(stream::once(async move { result }))
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
            base_url: normalize_openai_base_url(&base_url.into()),
            api_key: api_key.into(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl Backend for OpenAiCompatibleBackend {
    async fn infer(&self, request: InferRequest) -> Result<InferResponse, LlmffError> {
        let url = format!("{}/chat/completions", self.base_url);
        let body = openai_request_body(&request, false);
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

    fn stream(&self, request: InferRequest) -> InferStream {
        let url = format!("{}/chat/completions", self.base_url);
        let api_key = self.api_key.clone();
        let client = self.client.clone();

        Box::pin(async_stream::try_stream! {
            let body = openai_request_body(&request, true);
            let mut http_request = client.post(url).json(&body);
            if !api_key.is_empty() {
                http_request = http_request.bearer_auth(&api_key);
            }

            let response = http_request
                .send()
                .await
                .map_err(|error| LlmffError::Backend(format!("request failed: {error}")))?;
            let status = response.status();
            if status.is_success() {
                let mut body_stream = response.bytes_stream();
                let mut buffer = String::new();
                while let Some(bytes) = body_stream.next().await {
                    let bytes = bytes
                        .map_err(|error| LlmffError::Backend(format!("stream read failed: {error}")))?;
                    let text = std::str::from_utf8(&bytes)
                        .map_err(|error| LlmffError::Backend(format!("stream chunk was not UTF-8: {error}")))?;
                    buffer.push_str(text);

                    while let Some(event) = take_sse_event(&mut buffer) {
                        for chunk in parse_openai_sse_event(&event)? {
                            yield chunk;
                        }
                    }
                }

                if !buffer.trim().is_empty() {
                    for chunk in parse_openai_sse_event(&buffer)? {
                        yield chunk;
                    }
                }
            } else {
                let body = response.text().await.unwrap_or_default();
                Err(LlmffError::Backend(format!(
                    "OpenAI-compatible backend returned {status}: {body}"
                )))?;
            }
        })
    }
}

fn openai_request_body(request: &InferRequest, stream: bool) -> serde_json::Value {
    let mut body = json!({
        "model": request.model,
        "messages": request.messages,
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
    if let Some(seed) = request.seed {
        body["seed"] = json!(seed);
    }
    if request.response_format.as_deref() == Some("json") {
        body["response_format"] = json!({ "type": "json_object" });
    }
    if !request.stop.is_empty() {
        body["stop"] = json!(request.stop);
    }
    if stream {
        body["stream"] = json!(true);
        body["stream_options"] = json!({ "include_usage": true });
    }

    body
}

fn normalize_openai_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.ends_with("/v1") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/v1")
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
        "messages": request.messages,
        "stream": false,
    });
    if request.response_format.as_deref() == Some("json") {
        body["format"] = json!("json");
    }

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
    if let Some(seed) = request.seed {
        options.insert("seed".to_string(), json!(seed));
    }
    if !request.stop.is_empty() {
        options.insert("stop".to_string(), json!(request.stop));
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
struct ChatCompletionStreamResponse {
    choices: Vec<ChatStreamChoice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatStreamChoice {
    delta: ChatDelta,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatDelta {
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

fn take_sse_event(buffer: &mut String) -> Option<String> {
    let (separator_index, separator_len) = if let Some(index) = buffer.find("\n\n") {
        (index, 2)
    } else if let Some(index) = buffer.find("\r\n\r\n") {
        (index, 4)
    } else {
        return None;
    };

    let event = buffer[..separator_index].to_string();
    buffer.drain(..separator_index + separator_len);
    Some(event)
}

fn parse_openai_sse_event(event: &str) -> Result<Vec<InferStreamChunk>, LlmffError> {
    let data = event
        .lines()
        .filter_map(|line| line.trim_end_matches('\r').strip_prefix("data:"))
        .map(str::trim_start)
        .collect::<Vec<_>>()
        .join("\n");
    if data.is_empty() {
        return Ok(Vec::new());
    }
    if data == "[DONE]" {
        return Ok(vec![InferStreamChunk {
            delta: String::new(),
            done: true,
            usage: None,
        }]);
    }

    let chunk: ChatCompletionStreamResponse = serde_json::from_str(&data)
        .map_err(|error| LlmffError::Backend(format!("invalid stream JSON: {error}")))?;
    let delta = chunk
        .choices
        .first()
        .and_then(|choice| choice.delta.content.as_deref())
        .unwrap_or_default()
        .to_string();
    if delta.is_empty() && chunk.usage.is_none() {
        return Ok(Vec::new());
    }

    Ok(vec![InferStreamChunk {
        delta,
        done: false,
        usage: chunk.usage.map(Into::into),
    }])
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
    use futures::StreamExt;

    #[test]
    fn builtin_backend_families_describe_capabilities() {
        let families = builtin_backend_families();

        assert_eq!(families[0].name, "mock");
        assert!(families[0].model_aliases.contains(&"mock:good"));
        assert!(families[0].capabilities.contains(&"deterministic"));

        let openai = families
            .iter()
            .find(|family| family.name == "openai-compatible")
            .expect("OpenAI-compatible backend family should be listed");
        assert_eq!(openai.kind, "remote-chat");
        assert_eq!(openai.registration_flag, "--backend <alias>=<base-url>");
        assert!(openai.requires_api_key);
        assert!(openai.capabilities.contains(&"usage-metadata"));
        assert!(openai.capabilities.contains(&"streaming-inference"));
    }

    #[tokio::test]
    async fn mock_backend_streams_configured_response_as_single_delta() {
        let backend = MockBackend::new("mock:json", r#"{"answer":"ok"}"#);
        let request = InferRequest {
            model: "mock:json".to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: "hello".to_string(),
            }],
            temperature: None,
            top_p: None,
            max_tokens: None,
            seed: None,
            response_format: None,
            stop: vec![],
        };

        let chunks = backend
            .stream(request)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .expect("mock stream should succeed");

        assert_eq!(
            chunks,
            vec![InferStreamChunk {
                delta: r#"{"answer":"ok"}"#.to_string(),
                done: true,
                usage: None,
            }]
        );
    }

    #[tokio::test]
    async fn openai_compatible_backend_streams_chat_completion_deltas() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(
                    r#"data: {"choices":[{"delta":{"content":"hel"}}]}

data: {"choices":[{"delta":{"content":"lo"}}],"usage":{"prompt_tokens":3,"completion_tokens":2,"total_tokens":5}}

data: [DONE]

"#,
                ),
            )
            .mount(&server)
            .await;

        let backend = OpenAiCompatibleBackend::new(server.uri(), "");
        let request = InferRequest {
            model: "test-model".to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: "Say hello".to_string(),
            }],
            temperature: None,
            top_p: None,
            max_tokens: None,
            seed: None,
            response_format: None,
            stop: vec![],
        };

        let chunks = backend
            .stream(request)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .expect("OpenAI-compatible stream should succeed");

        assert_eq!(
            chunks,
            vec![
                InferStreamChunk {
                    delta: "hel".to_string(),
                    done: false,
                    usage: None,
                },
                InferStreamChunk {
                    delta: "lo".to_string(),
                    done: false,
                    usage: Some(UsageMetadata {
                        prompt_tokens: Some(3),
                        completion_tokens: Some(2),
                        total_tokens: Some(5),
                    }),
                },
                InferStreamChunk {
                    delta: String::new(),
                    done: true,
                    usage: None,
                },
            ]
        );

        let requests = server.received_requests().await.unwrap();
        let body: serde_json::Value = requests[0].body_json().unwrap();
        assert_eq!(body["stream"], true);
    }

    #[test]
    fn normalizes_openai_base_url_to_api_root() {
        assert_eq!(
            normalize_openai_base_url("https://api.openai.com"),
            "https://api.openai.com/v1"
        );
        assert_eq!(
            normalize_openai_base_url("https://api.openai.com/v1"),
            "https://api.openai.com/v1"
        );
        assert_eq!(
            normalize_openai_base_url("https://api.openai.com/v1/"),
            "https://api.openai.com/v1"
        );
    }

    #[tokio::test]
    async fn mock_backend_returns_configured_response() {
        let backend = MockBackend::new("mock:json", r#"{"answer":"ok"}"#);
        let response = backend
            .infer(InferRequest {
                model: "mock:json".to_string(),
                messages: vec![Message {
                    role: "user".to_string(),
                    content: "Return JSON".to_string(),
                }],
                temperature: Some(0.2),
                top_p: None,
                max_tokens: None,
                seed: None,
                response_format: None,
                stop: Vec::new(),
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
                messages: vec![
                    Message {
                        role: "system".to_string(),
                        content: "Use terse JSON.".to_string(),
                    },
                    Message {
                        role: "user".to_string(),
                        content: "Say hello".to_string(),
                    },
                ],
                temperature: Some(0.0),
                top_p: Some(0.9),
                max_tokens: Some(256),
                seed: Some(12345),
                response_format: Some("json".to_string()),
                stop: vec!["\nEND".to_string(), "</answer>".to_string()],
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
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][0]["content"], "Use terse JSON.");
        assert_eq!(body["messages"][1]["role"], "user");
        assert_eq!(body["messages"][1]["content"], "Say hello");
        assert!((body["top_p"].as_f64().unwrap() - 0.9).abs() < 0.000_001);
        assert_eq!(body["max_tokens"], 256);
        assert_eq!(body["seed"], 12345);
        assert_eq!(body["response_format"]["type"], "json_object");
        assert_eq!(body["stop"], serde_json::json!(["\nEND", "</answer>"]));
    }

    #[tokio::test]
    async fn openai_compatible_backend_accepts_versioned_base_url() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [
                    {
                        "message": {
                            "content": "hello from versioned base"
                        }
                    }
                ]
            })))
            .mount(&server)
            .await;

        let backend = OpenAiCompatibleBackend::new(format!("{}/v1", server.uri()), "");
        let response = backend
            .infer(InferRequest {
                model: "test-model".to_string(),
                messages: vec![Message {
                    role: "user".to_string(),
                    content: "Say hello".to_string(),
                }],
                temperature: None,
                top_p: None,
                max_tokens: None,
                seed: None,
                response_format: None,
                stop: Vec::new(),
            })
            .await
            .unwrap();

        assert_eq!(response.text, "hello from versioned base");
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
                messages: vec![
                    Message {
                        role: "system".to_string(),
                        content: "Use terse JSON.".to_string(),
                    },
                    Message {
                        role: "user".to_string(),
                        content: "Say hello".to_string(),
                    },
                ],
                temperature: Some(0.2),
                top_p: Some(0.8),
                max_tokens: Some(128),
                seed: Some(12345),
                response_format: Some("json".to_string()),
                stop: vec!["END".to_string(), "DONE".to_string()],
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
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][0]["content"], "Use terse JSON.");
        assert_eq!(body["messages"][1]["role"], "user");
        assert_eq!(body["messages"][1]["content"], "Say hello");
        assert_eq!(body["stream"], false);
        assert!((body["options"]["temperature"].as_f64().unwrap() - 0.2).abs() < 0.000_001);
        assert!((body["options"]["top_p"].as_f64().unwrap() - 0.8).abs() < 0.000_001);
        assert_eq!(body["options"]["num_predict"], 128);
        assert_eq!(body["options"]["seed"], 12345);
        assert_eq!(body["format"], "json");
        assert_eq!(body["options"]["stop"], serde_json::json!(["END", "DONE"]));
    }
}
