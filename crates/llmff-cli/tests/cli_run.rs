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
fn backends_list_prints_ollama_backend() {
    let mut cmd = Command::cargo_bin("llmff").unwrap();

    cmd.args(["backends", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ollama"));
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
fn run_reports_invalid_json_input_format() {
    let dir = tempfile::tempdir().unwrap();
    let payload = dir.path().join("payload.json");
    let output = dir.path().join("selected.txt");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&payload, "{not-json").unwrap();
    std::fs::write(
        &manifest,
        format!(
            r#"
version: 1
inputs:
  payload:
    path: {}
    format: json
graph:
  - id: load_payload
    op: load
    input: payload
outputs:
  final:
    from: load_payload
    path: {}
"#,
            payload.display(),
            output.display()
        ),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("llmff").unwrap();
    cmd.args(["run", manifest.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "input `payload` is not valid JSON",
        ));
}

#[test]
fn run_routes_json_input_by_field() {
    let dir = tempfile::tempdir().unwrap();
    let payload = dir.path().join("payload.json");
    let template = dir.path().join("simple.tmpl");
    let output = dir.path().join("selected.txt");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&payload, r#"{"kind":"simple","answer":"ok"}"#).unwrap();
    std::fs::write(&template, "{{answer}}").unwrap();
    std::fs::write(
        &manifest,
        format!(
            r#"
version: 1
inputs:
  payload:
    path: {}
    format: json
graph:
  - id: load_payload
    op: load
    input: payload
  - id: simple_answer
    op: template
    from: load_payload
    path: {}
  - id: choose
    op: route
    from: load_payload
    field: kind
    cases:
      simple: simple_answer
outputs:
  final:
    from: choose
    path: {}
"#,
            payload.display(),
            template.display(),
            output.display()
        ),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("llmff").unwrap();
    cmd.args(["run", manifest.to_str().unwrap()])
        .assert()
        .success();

    assert_eq!(std::fs::read_to_string(output).unwrap(), "ok");
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
    cmd.args(["run", manifest.to_str().unwrap(), "-g", "load | write(-)"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "provide either manifest or --graph",
        ));
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

#[test]
fn inspect_rejects_unregistered_backend_alias() {
    let dir = tempfile::tempdir().unwrap();
    let prompt = dir.path().join("question.txt");
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
"#,
            prompt.display()
        ),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("llmff").unwrap();
    cmd.args(["inspect", manifest.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "no backend configured for `openai:gpt-test`",
        ));
}

#[test]
fn inspect_accepts_registered_openai_backend_without_calling_server() {
    let dir = tempfile::tempdir().unwrap();
    let prompt = dir.path().join("question.txt");
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
"#,
            prompt.display()
        ),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("llmff").unwrap();
    cmd.args([
        "inspect",
        manifest.to_str().unwrap(),
        "--backend",
        "openai=http://127.0.0.1:1",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("ok"));
}

#[test]
fn inspect_rejects_field_route_from_text_source() {
    let dir = tempfile::tempdir().unwrap();
    let prompt = dir.path().join("question.txt");
    let template = dir.path().join("fast.tmpl");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&prompt, r#"{"kind":"simple"}"#).unwrap();
    std::fs::write(&template, "fast").unwrap();
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
  - id: fast_answer
    op: template
    from: load_prompt
    path: {}
  - id: choose
    op: route
    from: load_prompt
    field: kind
    cases:
      simple: fast_answer
"#,
            prompt.display(),
            template.display()
        ),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("llmff").unwrap();
    cmd.args(["inspect", manifest.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "field route requires JSON source `load_prompt`, got text",
        ));
}

#[test]
fn inspect_accepts_field_route_from_json_input() {
    let dir = tempfile::tempdir().unwrap();
    let payload = dir.path().join("payload.json");
    let template = dir.path().join("simple.tmpl");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&payload, r#"{"kind":"simple","answer":"ok"}"#).unwrap();
    std::fs::write(&template, "{{answer}}").unwrap();
    std::fs::write(
        &manifest,
        format!(
            r#"
version: 1
inputs:
  payload:
    path: {}
    format: json
graph:
  - id: load_payload
    op: load
    input: payload
  - id: simple_answer
    op: template
    from: load_payload
    path: {}
  - id: choose
    op: route
    from: load_payload
    field: kind
    cases:
      simple: simple_answer
"#,
            payload.display(),
            template.display()
        ),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("llmff").unwrap();
    cmd.args(["inspect", manifest.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("ok"));
}

#[test]
fn inspect_rejects_unknown_when_condition() {
    let dir = tempfile::tempdir().unwrap();
    let prompt = dir.path().join("question.txt");
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
    when: maybe
    model: mock:good
"#,
            prompt.display()
        ),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("llmff").unwrap();
    cmd.args(["inspect", manifest.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown when condition `maybe`"));
}

#[test]
fn inspect_rejects_unknown_input_format() {
    let dir = tempfile::tempdir().unwrap();
    let payload = dir.path().join("payload.json");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&payload, r#"{"kind":"simple"}"#).unwrap();
    std::fs::write(
        &manifest,
        format!(
            r#"
version: 1
inputs:
  payload:
    path: {}
    format: yaml
graph:
  - id: load_payload
    op: load
    input: payload
"#,
            payload.display()
        ),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("llmff").unwrap();
    cmd.args(["inspect", manifest.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "input `payload` has unsupported format `yaml`",
        ));
}

#[test]
fn trace_command_summarizes_trace_jsonl() {
    let dir = tempfile::tempdir().unwrap();
    let trace = dir.path().join("trace.jsonl");
    std::fs::write(
        &trace,
        r#"{"run_id":"test-run","event":"stage_finished","stage_id":"draft","op":"infer","status":"success","timestamp_ms":1,"duration_ms":14,"model":"openai:gpt-test","backend":"openai","provider_model":"gpt-test"}
{"run_id":"test-run","event":"stage_finished","stage_id":"validate","op":"validate_json","status":"invalid","timestamp_ms":2,"duration_ms":1,"validation_errors":["missing answer"]}
{"run_id":"test-run","event":"run_finished","status":"succeeded","timestamp_ms":3}
"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("llmff").unwrap();
    cmd.args(["trace", trace.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("draft infer success 14ms"))
        .stdout(predicate::str::contains("model=openai:gpt-test"))
        .stdout(predicate::str::contains("backend=openai"))
        .stdout(predicate::str::contains("provider_model=gpt-test"))
        .stdout(predicate::str::contains(
            "validate validate_json invalid 1ms validation_errors=1",
        ))
        .stdout(predicate::str::contains("run test-run succeeded"))
        .stdout(predicate::str::contains("missing answer").not());
}

#[test]
fn trace_command_reports_invalid_json_line() {
    let dir = tempfile::tempdir().unwrap();
    let trace = dir.path().join("trace.jsonl");
    std::fs::write(
        &trace,
        r#"{"run_id":"test-run","event":"run_started","timestamp_ms":1}
not-json
"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("llmff").unwrap();
    cmd.args(["trace", trace.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid trace JSON on line 2"));
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
    model: ollama:test-model
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
        "--ollama",
        &format!("ollama={}", server.uri()),
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
