mod common;

use assert_cmd::Command;
use common::*;
use predicates::prelude::*;

#[test]
fn run_writes_event_stream_to_file() {
    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("answer.json");
    let events = dir.path().join("events.jsonl");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&prompt, "Return an answer object").unwrap();
    write_file(
        &manifest,
        infer_manifest(prompt.display(), "mock:good", output.display()),
    );

    let mut cmd = llmff_cmd();
    cmd.args([
        "run",
        "--events",
        events.to_str().unwrap(),
        manifest.to_str().unwrap(),
    ])
    .env("LLMFF_MOCK_GOOD_RESPONSE", r#"{"answer":"ok"}"#)
    .assert()
    .success();

    let lines = read_file(events);
    let event_names = lines
        .lines()
        .map(|line| {
            let event: serde_json::Value = serde_json::from_str(line).unwrap();
            event["event"].as_str().unwrap().to_string()
        })
        .collect::<Vec<_>>();

    assert_eq!(
        event_names,
        vec![
            "run_started",
            "stage_started",
            "stage_finished",
            "stage_started",
            "stage_finished",
            "run_finished"
        ]
    );
}

#[test]
fn run_streams_events_to_stdout() {
    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("answer.json");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&prompt, "Return an answer object").unwrap();
    write_file(
        &manifest,
        infer_manifest(prompt.display(), "mock:good", output.display()),
    );

    let mut cmd = llmff_cmd();
    let stdout = cmd
        .args(["run", "--events", "-", manifest.to_str().unwrap()])
        .env("LLMFF_MOCK_GOOD_RESPONSE", r#"{"answer":"ok"}"#)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let events = String::from_utf8(stdout).unwrap();

    assert!(events
        .lines()
        .any(|line| line.contains(r#""event":"stage_finished""#)));
    assert_eq!(read_file(output), r#"{"answer":"ok"}"#);
}

#[test]
fn run_writes_failure_event_to_event_stream_without_stdout_payload() {
    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let events = dir.path().join("events.jsonl");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&prompt, "do not leak this prompt").unwrap();
    write_file(
        &manifest,
        infer_manifest(prompt.display(), "missing:model", "answer.txt"),
    );

    let mut cmd = llmff_cmd();
    let output = cmd
        .args([
            "run",
            "--events",
            events.to_str().unwrap(),
            manifest.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .get_output()
        .clone();

    assert!(output.stdout.is_empty());
    let event_text = read_file(events);
    let event = event_text
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .find(|event| event["event"] == "run_failed")
        .expect("run_failed event should be emitted");

    assert_eq!(event["status"], "failed");
    assert_eq!(event["failure_kind"], "backend");
    assert_eq!(event["failure_message"], "backend request failed");
    assert!(!event_text.contains("do not leak this prompt"));
    assert!(!event_text.contains("missing:model"));
}

#[test]
fn run_rejects_stream_stage_with_events_stdout() {
    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("answer.txt");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&prompt, "Say hello").unwrap();
    write_file(
        &manifest,
        infer_manifest(prompt.display(), "mock:good", output.display()),
    );

    let mut cmd = llmff_cmd();
    cmd.args([
        "run",
        manifest.to_str().unwrap(),
        "--stream-stage",
        "draft",
        "--events",
        "-",
    ])
    .env("LLMFF_MOCK_GOOD_RESPONSE", "hello")
    .assert()
    .failure()
    .stderr(predicate::str::contains(
        "stream-stage cannot write to stdout while events stream to stdout",
    ));
}

#[test]
fn run_rejects_events_stdout_with_output_stdout() {
    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&prompt, "Say hello").unwrap();
    write_file(
        &manifest,
        infer_manifest(prompt.display(), "mock:good", "\"-\""),
    );

    let mut cmd = llmff_cmd();
    cmd.args(["run", "--events", "-", manifest.to_str().unwrap()])
        .env("LLMFF_MOCK_GOOD_RESPONSE", "hello")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "events cannot stream to stdout while manifest outputs write to stdout",
        ));
}

#[test]
fn run_rejects_stream_stage_with_output_stdout() {
    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&prompt, "Say hello").unwrap();
    write_file(
        &manifest,
        infer_manifest(prompt.display(), "mock:good", "\"-\""),
    );

    let mut cmd = llmff_cmd();
    cmd.args(["run", manifest.to_str().unwrap(), "--stream-stage", "draft"])
        .env("LLMFF_MOCK_GOOD_RESPONSE", "hello")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "stream-stage cannot write to stdout while manifest outputs write to stdout",
        ));
}

#[test]
fn run_streams_load_stage_payload_to_stdout() {
    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("answer.txt");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&prompt, "stream this input").unwrap();
    write_file(
        &manifest,
        infer_manifest(prompt.display(), "mock:good", output.display()),
    );

    let mut cmd = llmff_cmd();
    cmd.args([
        "run",
        manifest.to_str().unwrap(),
        "--stream-stage",
        "load_prompt",
    ])
    .env("LLMFF_MOCK_GOOD_RESPONSE", "final answer")
    .assert()
    .success()
    .stdout("stream this input");

    assert_eq!(read_file(output), "final answer");
}

#[test]
fn run_streams_retrieve_stage_payload_to_stdout() {
    let dir = temp_dir();
    let docs = dir.path().join("docs");
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("matches.json");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::create_dir(&docs).unwrap();
    std::fs::write(&prompt, "rust graph").unwrap();
    write_file(
        docs.join("rust.txt"),
        "Rust builds reliable graph pipelines.",
    );
    write_file(
        docs.join("python.txt"),
        "Python scripts are useful for quick notebooks.",
    );
    write_file(
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
  - id: retrieve_context
    op: retrieve
    from: load_prompt
    documents:
      - docs/python.txt
      - docs/rust.txt
    top_k: 1
outputs:
  final:
    from: retrieve_context
    path: {}
"#,
            prompt.display(),
            output.display()
        ),
    );

    let mut cmd = llmff_cmd();
    let stdout = cmd
        .args([
            "run",
            manifest.to_str().unwrap(),
            "--stream-stage",
            "retrieve_context",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let streamed: serde_json::Value =
        serde_json::from_slice(&stdout).expect("streamed retrieve output should be JSON");

    assert_eq!(streamed["query"], "rust graph");
    assert_eq!(streamed["matches"].as_array().unwrap().len(), 1);
    assert_eq!(streamed["matches"][0]["path"], "docs/rust.txt");
    assert_eq!(streamed["matches"][0]["score"], 2);
    assert_eq!(read_file(output), String::from_utf8(stdout).unwrap());
}

#[test]
fn events_streaming_smoke_fixture_passes() {
    let root = workspace_root();
    let script = root.join("scripts/smoke-events-streaming.sh");
    let binary = assert_cmd::cargo::cargo_bin("llmff");

    Command::new("bash")
        .arg(script)
        .env("LLMFF_BIN", binary)
        .assert()
        .success();
}

#[test]
fn observability_export_scripts_summarize_trace_fixture() {
    let root = workspace_root();
    let fixture = root.join("examples/supervision/fixtures/success-trace.jsonl");
    let failure_fixture = root.join("examples/supervision/fixtures/backend-error-trace.jsonl");
    let summary_script = root.join("scripts/trace-to-summary.sh");
    let metrics_script = root.join("scripts/trace-to-metrics.sh");

    Command::new("bash")
        .args([summary_script.to_str().unwrap(), fixture.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("run fixture-run success"))
        .stdout(predicate::str::contains(
            "stages total=5 success=5 failed=0",
        ))
        .stdout(predicate::str::contains(
            "timing run_wall_ms=54 total_stage_ms=48",
        ))
        .stdout(predicate::str::contains("artifacts outputs=1 caches=2"))
        .stdout(predicate::str::contains(
            "artifact output stage=write_answer path=examples/out/answer.json",
        ))
        .stdout(predicate::str::contains(
            "artifact cache stage=cached path=.llmff/cache/fixture.json hit=true",
        ))
        .stdout(predicate::str::contains(
            "tokens prompt=12 completion=8 total=20",
        ))
        .stdout(predicate::str::contains(
            "cache hits=1 misses=1 hit_rate=50.00%",
        ))
        .stdout(predicate::str::contains(
            "backend_errors total=0 rate=0.00%",
        ))
        .stdout(predicate::str::contains(
            "retries total=2 stages=1 max_attempts=3",
        ));

    Command::new("bash")
        .args([
            summary_script.to_str().unwrap(),
            failure_fixture.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("run fixture-error failed"))
        .stdout(predicate::str::contains(
            "timing run_wall_ms=5 total_stage_ms=0",
        ))
        .stdout(predicate::str::contains(
            "failures total=1 backend=1 timeout=0",
        ))
        .stdout(predicate::str::contains(
            "failure kind=backend message=backend request failed",
        ));

    Command::new("bash")
        .args([metrics_script.to_str().unwrap(), fixture.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("llmff_run_duration_ms 54"))
        .stdout(predicate::str::contains("llmff_stage_duration_ms_sum 48"))
        .stdout(predicate::str::contains("llmff_tokens_total 20"))
        .stdout(predicate::str::contains("llmff_cache_hit_rate 0.5000"))
        .stdout(predicate::str::contains("llmff_failures_total 0"))
        .stdout(predicate::str::contains("llmff_retries_total 2"))
        .stdout(predicate::str::contains("llmff_retry_stages_total 1"))
        .stdout(predicate::str::contains("llmff_max_stage_attempts 3"))
        .stdout(predicate::str::contains("llmff_timeout_errors_total 0"))
        .stdout(predicate::str::contains("llmff_timeout_error_rate 0.0000"))
        .stdout(predicate::str::contains("llmff_backend_error_rate 0.0000"));

    Command::new("bash")
        .args([
            metrics_script.to_str().unwrap(),
            failure_fixture.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("llmff_run_duration_ms 5"))
        .stdout(predicate::str::contains("llmff_failures_total 1"))
        .stdout(predicate::str::contains("llmff_timeout_errors_total 0"))
        .stdout(predicate::str::contains("llmff_timeout_error_rate 0.0000"))
        .stdout(predicate::str::contains("llmff_backend_error_rate 1.0000"));
}

#[test]
fn trace_command_summarizes_trace_jsonl() {
    let dir = temp_dir();
    let trace = dir.path().join("trace.jsonl");
    write_file(
        &trace,
        r#"{"run_id":"test-run","event":"stage_finished","stage_id":"draft","op":"infer","status":"success","timestamp_ms":1,"duration_ms":14,"model":"openai:gpt-test","backend":"openai","provider_model":"gpt-test","prompt_tokens":12,"completion_tokens":8,"total_tokens":20}
{"run_id":"test-run","event":"stage_finished","stage_id":"validate","op":"validate_json","status":"invalid","timestamp_ms":2,"duration_ms":1,"validation_errors":["missing answer"]}
{"run_id":"test-run","event":"stage_finished","stage_id":"cached","op":"cache","status":"success","timestamp_ms":3,"duration_ms":1,"cache_hit":true,"cache_path":".llmff/cache"}
{"run_id":"test-run","event":"run_finished","status":"succeeded","timestamp_ms":3}
"#,
    );

    let mut cmd = llmff_cmd();
    cmd.args(["trace", trace.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("draft infer success 14ms"))
        .stdout(predicate::str::contains("model=openai:gpt-test"))
        .stdout(predicate::str::contains("backend=openai"))
        .stdout(predicate::str::contains("provider_model=gpt-test"))
        .stdout(predicate::str::contains("usage=20"))
        .stdout(predicate::str::contains("prompt_tokens=12"))
        .stdout(predicate::str::contains("completion_tokens=8"))
        .stdout(predicate::str::contains(
            "validate validate_json invalid 1ms validation_errors=1",
        ))
        .stdout(predicate::str::contains(
            "cached cache success 1ms cache_hit=true cache_path=.llmff/cache",
        ))
        .stdout(predicate::str::contains("run test-run succeeded"))
        .stdout(predicate::str::contains("missing answer").not());
}

#[test]
fn trace_command_reports_invalid_json_line() {
    let dir = temp_dir();
    let trace = dir.path().join("trace.jsonl");
    write_file(
        &trace,
        r#"{"run_id":"test-run","event":"run_started","timestamp_ms":1}
not-json
"#,
    );

    let mut cmd = llmff_cmd();
    cmd.args(["trace", trace.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid trace JSON on line 2"));
}
