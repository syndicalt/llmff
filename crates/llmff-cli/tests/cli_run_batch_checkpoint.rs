mod common;

use common::*;
use predicates::prelude::*;

#[test]
fn run_batch_input_writes_isolated_item_outputs_and_report() {
    let dir = temp_dir();
    let batch_input = dir.path().join("batch.txt");
    let batch_output = dir.path().join("batch-out");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&batch_input, "first\nsecond\n").unwrap();
    write_file(
        &manifest,
        r#"
version: 1
inputs:
  prompt:
    path: placeholder.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
outputs:
  final:
    from: load_prompt
    path: answer.txt
"#,
    );

    llmff_cmd()
        .args([
            "run",
            "--batch-input",
            batch_input.to_str().unwrap(),
            "--batch-output-dir",
            batch_output.to_str().unwrap(),
            manifest.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert_eq!(
        read_text_artifact(&batch_output, "items/000000/answer.txt"),
        "first"
    );
    assert_eq!(
        read_text_artifact(&batch_output, "items/000001/answer.txt"),
        "second"
    );
    let report = read_text_artifact(&batch_output, "batch-report.jsonl");
    assert!(report.contains(r#""index":0"#));
    assert!(report.contains(r#""index":1"#));
    assert!(report.contains(r#""status":"succeeded""#));
}

#[test]
fn run_dir_batch_input_writes_supervisor_artifacts_and_batch_outputs() {
    let dir = temp_dir();
    let batch_input = dir.path().join("batch.txt");
    let batch_output = dir.path().join("batch-out");
    let manifest = dir.path().join("pipeline.yaml");
    let run_dir = dir.path().join("run");
    std::fs::write(&batch_input, "first\nsecond\n").unwrap();
    write_file(
        &manifest,
        r#"
version: 1
inputs:
  prompt:
    path: placeholder.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
outputs:
  final:
    from: load_prompt
    path: answer.txt
"#,
    );

    llmff_cmd()
        .args([
            "run",
            "--run-dir",
            run_dir.to_str().unwrap(),
            "--batch-input",
            batch_input.to_str().unwrap(),
            "--batch-output-dir",
            batch_output.to_str().unwrap(),
            manifest.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert!(run_dir.join("inspect.json").exists());
    assert!(run_dir.join("trace.jsonl").exists());
    assert!(run_dir.join("events.jsonl").exists());
    assert!(run_dir.join("checkpoint.json").exists());
    let result: serde_json::Value = read_run_result(&run_dir);
    assert_eq!(result["status"], "succeeded");
    assert_eq!(result["exit_code"], 0);
    assert_eq!(result["failure"], serde_json::Value::Null);

    assert_eq!(
        read_text_artifact(&batch_output, "items/000000/answer.txt"),
        "first"
    );
    assert_eq!(
        read_text_artifact(&batch_output, "items/000001/answer.txt"),
        "second"
    );
    let report = read_text_artifact(&batch_output, "batch-report.jsonl");
    assert!(report.contains(r#""status":"succeeded""#));
    let trace = read_text_artifact(&run_dir, "trace.jsonl");
    assert!(trace.contains(r#""event":"batch_item_finished""#));
    assert!(trace.contains(r#""status":"success""#));
    let checkpoint = read_text_artifact(&run_dir, "checkpoint.json");
    assert!(checkpoint.contains(r#""batch:000000""#));
    assert!(checkpoint.contains(r#""batch:000001""#));
}

#[test]
fn run_dir_batch_failure_writes_failed_result_summary() {
    let dir = temp_dir();
    let batch_input = dir.path().join("batch.txt");
    let batch_output = dir.path().join("batch-out");
    let tool = dir.path().join("batch-tool");
    let manifest = dir.path().join("pipeline.yaml");
    let run_dir = dir.path().join("run");
    std::fs::write(&batch_input, "first\nfail\n").unwrap();
    write_file(
        &tool,
        r#"#!/bin/sh
payload=$(cat)
if [ "$payload" = "fail" ]; then
  printf 'item failed\n' >&2
  exit 7
fi
printf '%s' "$payload"
"#,
    );
    make_executable(&tool);
    write_file(
        &manifest,
        format!(
            r#"
version: 1
inputs:
  prompt:
    path: placeholder.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: call_tool
    op: tool
    from: load_prompt
    command: [{}]
outputs:
  final:
    from: call_tool
    path: answer.txt
"#,
            tool.display()
        ),
    );

    llmff_cmd()
        .args([
            "run",
            "--run-dir",
            run_dir.to_str().unwrap(),
            "--batch-input",
            batch_input.to_str().unwrap(),
            "--batch-output-dir",
            batch_output.to_str().unwrap(),
            manifest.to_str().unwrap(),
        ])
        .assert()
        .code(20)
        .stderr(predicate::str::contains("one or more batch items failed"));

    let result: serde_json::Value = read_run_result(&run_dir);
    assert_eq!(result["status"], "failed");
    assert_eq!(result["exit_code"], 20);
    assert_eq!(result["failure"]["kind"], "stage_execution");
    assert_eq!(
        result["failure"]["retry_recommendation"],
        "check_stage_or_input"
    );

    let events = read_text_artifact(&run_dir, "events.jsonl");
    assert!(events.contains(r#""event":"run_started""#));
    assert!(events.contains(r#""event":"run_failed""#));
    assert!(events.contains(r#""failure_kind":"stage_execution""#));

    let report = read_text_artifact(&batch_output, "batch-report.jsonl");
    assert!(report.contains(r#""index":0"#));
    assert!(report.contains(r#""index":1"#));
    assert!(report.contains(r#""status":"succeeded""#));
    assert!(report.contains(r#""status":"failed""#));
    assert!(report.contains(r#""exit_code":20"#));
    assert!(report.contains(r#""failure_kind":"stage_execution""#));
    assert!(report.contains(r#""retry_recommendation":"check_stage_or_input""#));
}

#[test]
fn run_batch_input_rejects_parent_directory_outputs() {
    let dir = temp_dir();
    let batch_input = dir.path().join("batch.txt");
    let batch_output = dir.path().join("batch-out");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&batch_input, "first\nsecond\n").unwrap();
    write_file(
        &manifest,
        r#"
version: 1
inputs:
  prompt:
    path: placeholder.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
outputs:
  final:
    from: load_prompt
    path: ../shared.txt
"#,
    );

    llmff_cmd()
        .args([
            "run",
            "--batch-input",
            batch_input.to_str().unwrap(),
            "--batch-output-dir",
            batch_output.to_str().unwrap(),
            manifest.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "batch mode output paths cannot contain parent directory components",
        ));
}

#[test]
fn run_dir_batch_timeout_failure_preserves_failure_class() {
    let dir = temp_dir();
    let batch_input = dir.path().join("batch.txt");
    let batch_output = dir.path().join("batch-out");
    let tool = dir.path().join("slow-tool");
    let manifest = dir.path().join("pipeline.yaml");
    let run_dir = dir.path().join("run");
    std::fs::write(&batch_input, "first\n").unwrap();
    write_file(
        &tool,
        r#"#!/bin/sh
cat >/dev/null
sleep 30
"#,
    );
    make_executable(&tool);
    write_file(
        &manifest,
        format!(
            r#"
version: 1
inputs:
  prompt:
    path: placeholder.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: call_tool
    op: tool
    from: load_prompt
    command: [{}]
    timeout_ms: 1
outputs:
  final:
    from: call_tool
    path: answer.txt
"#,
            tool.display()
        ),
    );

    llmff_cmd()
        .args([
            "run",
            "--run-dir",
            run_dir.to_str().unwrap(),
            "--batch-input",
            batch_input.to_str().unwrap(),
            "--batch-output-dir",
            batch_output.to_str().unwrap(),
            manifest.to_str().unwrap(),
        ])
        .assert()
        .code(21)
        .stderr(predicate::str::contains("one or more batch items failed"));

    let result: serde_json::Value = read_run_result(&run_dir);
    assert_eq!(result["status"], "failed");
    assert_eq!(result["exit_code"], 21);
    assert_eq!(result["failure"]["kind"], "timeout");
    assert_eq!(
        result["failure"]["retry_recommendation"],
        "check_stage_or_input"
    );

    let report = read_text_artifact(&batch_output, "batch-report.jsonl");
    assert!(report.contains(r#""exit_code":21"#));
    assert!(report.contains(r#""failure_kind":"timeout""#));
    assert!(report.contains(r#""retry_recommendation":"check_stage_or_input""#));
    let trace = read_text_artifact(&run_dir, "trace.jsonl");
    assert!(trace.contains(r#""event":"batch_item_finished""#));
    assert!(trace.contains(r#""failure_kind":"timeout""#));
}

#[test]
fn run_batch_input_rejects_unsupported_supervision_flags() {
    let dir = temp_dir();
    let batch_input = dir.path().join("batch.txt");
    let batch_output = dir.path().join("batch-out");
    let events = dir.path().join("events.jsonl");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&batch_input, "first\nsecond\n").unwrap();
    write_file(
        &manifest,
        r#"
version: 1
inputs:
  prompt:
    path: placeholder.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
outputs:
  final:
    from: load_prompt
    path: answer.txt
"#,
    );

    llmff_cmd()
        .args([
            "run",
            "--batch-input",
            batch_input.to_str().unwrap(),
            "--batch-output-dir",
            batch_output.to_str().unwrap(),
            "--events",
            events.to_str().unwrap(),
            manifest.to_str().unwrap(),
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "batch mode does not support explicit trace, events, checkpoint, resume, replay-trace, or stream-stage flags",
        ));

    assert!(!events.exists());
    assert!(!batch_output.exists());
}
