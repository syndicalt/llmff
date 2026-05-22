use assert_cmd::Command;
use predicates::prelude::*;
use std::path::PathBuf;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("CLI crate should live under crates/llmff-cli")
        .to_path_buf()
}

#[test]
fn stages_list_prints_builtin_stages() {
    let mut cmd = Command::cargo_bin("llmff").unwrap();

    cmd.args(["stages", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("infer"))
        .stdout(predicate::str::contains("validate_json"));
}

#[test]
fn run_executes_manifest_with_mock_backends() {
    let dir = tempfile::tempdir().unwrap();
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("answer.json");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&prompt, "Return an answer object").unwrap();
    std::fs::write(
        &manifest,
        format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: load_prompt
    model: mock:bad
  - id: validate
    op: validate_json
    from: draft
    schema: '{{"type":"object","required":["answer"]}}'
  - id: repair
    op: repair
    from: validate
    when: invalid
    model: mock:good
outputs:
  final:
    from: repair
    path: {}
"#,
            prompt.display(),
            output.display()
        ),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("llmff").unwrap();
    cmd.args(["run", manifest.to_str().unwrap()])
        .env("LLMFF_MOCK_BAD_RESPONSE", r#"{"wrong":true}"#)
        .env("LLMFF_MOCK_GOOD_RESPONSE", r#"{"answer":"ok"}"#)
        .assert()
        .success();

    assert_eq!(
        std::fs::read_to_string(output).unwrap(),
        r#"{"answer":"ok"}"#
    );
}

#[test]
fn run_supports_stdin_and_stdout_paths() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(
        &manifest,
        r#"
version: 1
inputs:
  prompt:
    path: "-"
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: load_prompt
    model: mock:good
outputs:
  final:
    from: draft
    path: "-"
"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("llmff").unwrap();
    cmd.args(["run", manifest.to_str().unwrap()])
        .write_stdin("Return JSON")
        .env("LLMFF_MOCK_GOOD_RESPONSE", r#"{"answer":"ok"}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#"{"answer":"ok"}"#));
}

#[test]
fn inline_graph_run_uses_input_graph_and_write_stage() {
    let dir = tempfile::tempdir().unwrap();
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("answer.json");
    std::fs::write(&prompt, "Return an answer object").unwrap();

    let mut cmd = Command::cargo_bin("llmff").unwrap();
    cmd.current_dir(dir.path())
        .args([
            "run",
            "-i",
            prompt.to_str().unwrap(),
            "-g",
            "load | infer(model=mock:good) | write(answer.json)",
        ])
        .env("LLMFF_MOCK_GOOD_RESPONSE", r#"{"answer":"ok"}"#)
        .assert()
        .success();

    assert_eq!(
        std::fs::read_to_string(output).unwrap(),
        r#"{"answer":"ok"}"#
    );
}

#[test]
fn inline_graph_run_defaults_load_to_stdin_and_write_to_stdout() {
    let mut cmd = Command::cargo_bin("llmff").unwrap();
    cmd.args(["run", "-g", "load | infer(model=mock:good) | write(-)"])
        .write_stdin("Return an answer object")
        .env("LLMFF_MOCK_GOOD_RESPONSE", r#"{"answer":"ok"}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#"{"answer":"ok"}"#));
}

#[test]
fn inline_graph_run_rejects_manifest_and_graph_together() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(
        &manifest,
        r#"
version: 1
graph: []
"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("llmff").unwrap();
    cmd.args([
        "run",
        manifest.to_str().unwrap(),
        "-g",
        "load | write(-)",
    ])
    .assert()
    .failure()
    .stderr(predicate::str::contains("provide either manifest or --graph"));
}

#[test]
fn inspect_example_manifest_succeeds() {
    let mut cmd = Command::cargo_bin("llmff").unwrap();

    cmd.args(["inspect", "examples/json-repair.yaml"])
        .current_dir(workspace_root())
        .assert()
        .success()
        .stdout(predicate::str::contains("ok"));
}

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

    let dir = tempfile::tempdir().unwrap();
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("answer.json");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&prompt, "Return an answer object").unwrap();
    std::fs::write(
        &manifest,
        format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: load_prompt
    model: openai:gpt-test
outputs:
  final:
    from: draft
    path: {}
"#,
            prompt.display(),
            output.display()
        ),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("llmff").unwrap();
    cmd.args([
        "run",
        manifest.to_str().unwrap(),
        "--backend",
        &format!("openai={}", server.uri()),
    ])
    .assert()
    .success();

    assert_eq!(
        std::fs::read_to_string(output).unwrap(),
        r#"{"answer":"ok"}"#
    );
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

    let dir = tempfile::tempdir().unwrap();
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("answer.json");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&prompt, "Return an answer object").unwrap();
    std::fs::write(
        &manifest,
        format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: load_prompt
    model: openai:gpt-test
outputs:
  final:
    from: draft
    path: {}
"#,
            prompt.display(),
            output.display()
        ),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("llmff").unwrap();
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

    assert_eq!(
        std::fs::read_to_string(output).unwrap(),
        r#"{"answer":"ok"}"#
    );
}

#[tokio::test]
async fn missing_api_key_env_reports_backend_alias_and_env_name() {
    let server = MockServer::start().await;
    let dir = tempfile::tempdir().unwrap();
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("answer.json");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&prompt, "Return an answer object").unwrap();
    std::fs::write(
        &manifest,
        format!(
            r#"
version: 1
inputs:
  prompt:
    path: {}
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: load_prompt
    model: openai:gpt-test
outputs:
  final:
    from: draft
    path: {}
"#,
            prompt.display(),
            output.display()
        ),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("llmff").unwrap();
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
