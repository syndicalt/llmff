mod common;

use common::*;
use predicates::prelude::*;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn run_uses_cli_registered_openai_backend() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "choices": [
                {
                    "message": {
                        "content": "{\"answer\":\"ok\"}"
                    }
                }
            ]
        })))
        .mount(&server)
        .await;

    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("answer.json");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&prompt, "Return an answer object").unwrap();
    write_file(
        &manifest,
        infer_manifest(prompt.display(), "openai:gpt-test", output.display()),
    );

    let mut cmd = llmff_cmd();
    cmd.args([
        "run",
        manifest.to_str().unwrap(),
        "--backend",
        &format!("openai={}", server.uri()),
    ])
    .assert()
    .success();

    assert_eq!(read_file(output), r#"{"answer":"ok"}"#);
}

#[tokio::test]
async fn run_streams_infer_stage_deltas_to_stdout() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            concat!(
                "data: {\"choices\":[{\"delta\":{\"content\":\"hello \"}}]}\n\n",
                "data: {\"choices\":[{\"delta\":{\"content\":\"world\"}}]}\n\n",
                "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":4,\"completion_tokens\":2,\"total_tokens\":6}}\n\n",
                "data: [DONE]\n\n",
            ),
        ))
        .mount(&server)
        .await;

    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("answer.txt");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&prompt, "Say hello").unwrap();
    write_file(
        &manifest,
        infer_manifest(prompt.display(), "openai:gpt-test", output.display()),
    );

    let mut cmd = llmff_cmd();
    cmd.args([
        "run",
        manifest.to_str().unwrap(),
        "--stream-stage",
        "draft",
        "--backend",
        &format!("openai={}", server.uri()),
    ])
    .assert()
    .success()
    .stdout("hello world");

    assert_eq!(read_file(output), "hello world");
}

#[tokio::test]
async fn run_accepts_cli_registered_openai_backend_with_v1_base_url() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "choices": [
                {
                    "message": {
                        "content": "{\"answer\":\"ok\"}"
                    }
                }
            ]
        })))
        .mount(&server)
        .await;

    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("answer.json");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&prompt, "Return an answer object").unwrap();
    write_file(
        &manifest,
        infer_manifest(prompt.display(), "openai:gpt-test", output.display()),
    );

    let mut cmd = llmff_cmd();
    cmd.args([
        "run",
        manifest.to_str().unwrap(),
        "--backend",
        &format!("openai={}/v1", server.uri()),
    ])
    .assert()
    .success();

    assert_eq!(read_file(output), r#"{"answer":"ok"}"#);
}

#[tokio::test]
async fn run_uses_cli_registered_ollama_backend() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "model": "test-model",
            "message": {
                "role": "assistant",
                "content": "{\"answer\":\"ok\"}"
            },
            "done": true
        })))
        .mount(&server)
        .await;

    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("answer.json");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&prompt, "Return an answer object").unwrap();
    write_file(
        &manifest,
        infer_manifest(prompt.display(), "ollama:test-model", output.display()),
    );

    let mut cmd = llmff_cmd();
    cmd.args([
        "run",
        manifest.to_str().unwrap(),
        "--ollama",
        &format!("ollama={}", server.uri()),
    ])
    .assert()
    .success();

    assert_eq!(read_file(output), r#"{"answer":"ok"}"#);
}

#[tokio::test]
async fn run_uses_api_key_env_without_printing_secret() {
    let secret = "llmff-test-secret";
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("authorization", format!("Bearer {secret}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "choices": [
                {
                    "message": {
                        "content": "{\"answer\":\"ok\"}"
                    }
                }
            ]
        })))
        .mount(&server)
        .await;

    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("answer.json");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&prompt, "Return an answer object").unwrap();
    write_file(
        &manifest,
        infer_manifest(prompt.display(), "openai:gpt-test", output.display()),
    );

    let mut cmd = llmff_cmd();
    cmd.args([
        "run",
        manifest.to_str().unwrap(),
        "--backend",
        &format!("openai={}", server.uri()),
        "--api-key-env",
        "openai=LLMFF_TEST_API_KEY",
    ])
    .env("LLMFF_TEST_API_KEY", secret)
    .assert()
    .success()
    .stdout(predicate::str::contains(secret).not())
    .stderr(predicate::str::contains(secret).not());

    assert_eq!(read_file(output), r#"{"answer":"ok"}"#);
}

#[tokio::test]
async fn missing_api_key_env_reports_backend_alias_and_env_name() {
    let server = MockServer::start().await;
    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("answer.json");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&prompt, "Return an answer object").unwrap();
    write_file(
        &manifest,
        infer_manifest(prompt.display(), "openai:gpt-test", output.display()),
    );

    let mut cmd = llmff_cmd();
    cmd.args([
        "run",
        manifest.to_str().unwrap(),
        "--backend",
        &format!("openai={}", server.uri()),
        "--api-key-env",
        "openai=LLMFF_MISSING_API_KEY",
    ])
    .env_remove("LLMFF_MISSING_API_KEY")
    .assert()
    .failure()
    .stderr(predicate::str::contains(
        "api key env `LLMFF_MISSING_API_KEY` for backend `openai` is not set",
    ));
}
