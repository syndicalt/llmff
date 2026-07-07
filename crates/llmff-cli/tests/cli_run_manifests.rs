mod common;

use common::*;
use predicates::prelude::*;
#[cfg(unix)]
use std::process::Command as StdCommand;
#[cfg(unix)]
use std::thread;
#[cfg(unix)]
use std::time::{Duration, Instant};

#[test]
fn run_executes_manifest_with_mock_backends() {
    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("answer.json");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&prompt, "Return an answer object").unwrap();
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
    );

    let mut cmd = llmff_cmd();
    cmd.args(["run", manifest.to_str().unwrap()])
        .env("LLMFF_MOCK_BAD_RESPONSE", r#"{"wrong":true}"#)
        .env("LLMFF_MOCK_GOOD_RESPONSE", r#"{"answer":"ok"}"#)
        .assert()
        .success();

    assert_eq!(read_file(output), r#"{"answer":"ok"}"#);
}

#[test]
fn run_dir_writes_supervisor_artifacts_and_result_summary() {
    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("answer.json");
    let manifest = dir.path().join("pipeline.yaml");
    let run_dir = dir.path().join("run");
    std::fs::write(&prompt, "Return an answer object").unwrap();
    write_file(
        &manifest,
        infer_manifest(prompt.display(), "mock:good", output.display()),
    );

    llmff_cmd()
        .args([
            "run",
            "--run-dir",
            run_dir.to_str().unwrap(),
            manifest.to_str().unwrap(),
        ])
        .env("LLMFF_MOCK_GOOD_RESPONSE", r#"{"answer":"ok"}"#)
        .assert()
        .success();

    assert!(run_dir.join("inspect.json").exists());
    assert!(run_dir.join("trace.jsonl").exists());
    assert!(run_dir.join("events.jsonl").exists());
    assert!(run_dir.join("checkpoint.json").exists());
    let result: serde_json::Value = read_run_result(&run_dir);

    assert_eq!(result["schema_version"], 1);
    assert_eq!(result["status"], "succeeded");
    assert_eq!(result["exit_code"], 0);
    assert!(result["manifest"]["hash"]
        .as_str()
        .expect("manifest hash should be a string")
        .starts_with("sha256:"));
    assert_eq!(result["artifacts"]["inspect"], "inspect.json");
    assert_eq!(result["artifacts"]["trace"], "trace.jsonl");
    assert_eq!(result["artifacts"]["events"], "events.jsonl");
    assert_eq!(result["artifacts"]["checkpoint"], "checkpoint.json");
    assert_eq!(result["failure"], serde_json::Value::Null);

    let inspect: serde_json::Value = read_json_artifact(&run_dir, "inspect.json");
    let checkpoint: serde_json::Value = read_json_artifact(&run_dir, "checkpoint.json");
    assert_eq!(result["manifest"]["hash"], inspect["manifest"]["hash"]);
    assert_eq!(
        result["manifest"]["hash"],
        format!("sha256:{}", checkpoint["manifest_hash"].as_str().unwrap())
    );
}

#[test]
fn run_dir_writes_failed_result_summary_for_supervisors() {
    let dir = temp_dir();
    let tool = dir.path().join("fail-tool");
    let manifest = dir.path().join("pipeline.yaml");
    let run_dir = dir.path().join("run");
    write_file(
        &tool,
        r#"#!/bin/sh
cat >/dev/null
printf 'tool failed\n' >&2
exit 7
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
    path: "-"
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
    path: "-"
"#,
            tool.display()
        ),
    );

    llmff_cmd()
        .args([
            "run",
            "--run-dir",
            run_dir.to_str().unwrap(),
            manifest.to_str().unwrap(),
        ])
        .write_stdin("payload")
        .assert()
        .code(20)
        .stderr(predicate::str::contains("tool command exited with status"));

    let result: serde_json::Value = read_run_result(&run_dir);
    assert_eq!(result["status"], "failed");
    assert_eq!(result["exit_code"], 20);
    assert_eq!(result["failure"]["kind"], "stage_execution");
    assert_eq!(
        result["failure"]["retry_recommendation"],
        "check_stage_or_input"
    );
    assert!(result["failure"]["message"]
        .as_str()
        .expect("failure message should be present")
        .contains("tool command exited with status"));
}

#[test]
fn run_dir_writes_result_summary_for_validation_failure() {
    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let manifest = dir.path().join("pipeline.yaml");
    let run_dir = dir.path().join("run");
    std::fs::write(&prompt, "Return an answer object").unwrap();
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
  - id: draft
    op: missing_op
    from: load_prompt
outputs:
  final:
    from: draft
    path: answer.json
"#,
            prompt.display()
        ),
    );

    llmff_cmd()
        .args([
            "run",
            "--run-dir",
            run_dir.to_str().unwrap(),
            manifest.to_str().unwrap(),
        ])
        .assert()
        .code(10)
        .stderr(predicate::str::contains("unknown stage operation"));

    let result: serde_json::Value = read_run_result(&run_dir);
    assert_eq!(result["status"], "failed");
    assert_eq!(result["exit_code"], 10);
    assert_eq!(result["failure"]["kind"], "unknown_stage");
    assert_eq!(
        result["failure"]["retry_recommendation"],
        "do_not_retry_without_changes"
    );
}

#[test]
fn run_dir_rejects_explicit_metadata_paths() {
    let dir = temp_dir();
    let manifest = dir.path().join("pipeline.yaml");
    let run_dir = dir.path().join("run");
    let trace = dir.path().join("trace.jsonl");
    write_file(
        &manifest,
        r#"
version: 1
graph:
  - id: load_prompt
    op: load
outputs:
  final:
    from: load_prompt
    path: "-"
"#,
    );

    llmff_cmd()
        .args([
            "run",
            "--run-dir",
            run_dir.to_str().unwrap(),
            "--trace",
            trace.to_str().unwrap(),
            manifest.to_str().unwrap(),
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "--run-dir owns trace, events, and checkpoint paths",
        ));

    assert!(!run_dir.exists());
}

#[test]
fn run_dir_writes_result_summary_for_manifest_parse_failure() {
    let dir = temp_dir();
    let manifest = dir.path().join("pipeline.yaml");
    let run_dir = dir.path().join("run");
    std::fs::write(&manifest, "version: [not valid").unwrap();

    llmff_cmd()
        .args([
            "run",
            "--run-dir",
            run_dir.to_str().unwrap(),
            manifest.to_str().unwrap(),
        ])
        .assert()
        .code(10)
        .stderr(predicate::str::contains("failed to parse manifest"));

    let result: serde_json::Value = read_run_result(&run_dir);
    assert_eq!(result["status"], "failed");
    assert_eq!(result["exit_code"], 10);
    assert_eq!(result["failure"]["kind"], "manifest_parse");
    assert!(result["manifest"]["hash"]
        .as_str()
        .expect("manifest hash should be present")
        .starts_with("sha256:"));
}

#[test]
fn run_dir_result_exit_code_matches_missing_manifest_exit() {
    let dir = temp_dir();
    let manifest = dir.path().join("missing.yaml");
    let run_dir = dir.path().join("run");

    llmff_cmd()
        .args([
            "run",
            "--run-dir",
            run_dir.to_str().unwrap(),
            manifest.to_str().unwrap(),
        ])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("No such file or directory"));

    let result: serde_json::Value = read_run_result(&run_dir);
    assert_eq!(result["status"], "failed");
    assert_eq!(result["exit_code"], 1);
    assert_eq!(result["failure"]["kind"], "config");
}

#[test]
fn run_dir_usage_errors_do_not_create_partial_artifact_directory() {
    let dir = temp_dir();
    let manifest = dir.path().join("pipeline.yaml");
    let run_dir = dir.path().join("run");
    write_file(
        &manifest,
        r#"
version: 1
graph:
  - id: load_prompt
    op: load
outputs:
  final:
    from: load_prompt
    path: "-"
"#,
    );

    llmff_cmd()
        .args([
            "run",
            "--run-dir",
            run_dir.to_str().unwrap(),
            "--timeout-ms",
            "0",
            manifest.to_str().unwrap(),
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "timeout-ms must be greater than 0",
        ));

    assert!(!run_dir.exists());
}

#[test]
fn run_dir_writes_result_summary_for_stdout_ownership_failure() {
    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let manifest = dir.path().join("pipeline.yaml");
    let run_dir = dir.path().join("run");
    std::fs::write(&prompt, "Return an answer object").unwrap();
    write_file(&manifest, load_only_manifest(prompt.display(), "\"-\""));

    llmff_cmd()
        .args([
            "run",
            "--run-dir",
            run_dir.to_str().unwrap(),
            "--stream-stage",
            "load_prompt",
            manifest.to_str().unwrap(),
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "stream-stage cannot write to stdout while manifest outputs write to stdout",
        ));

    let result: serde_json::Value = read_run_result(&run_dir);
    assert_eq!(result["status"], "failed");
    assert_eq!(result["exit_code"], 2);
    assert_eq!(result["failure"]["kind"], "config");
}

#[test]
fn run_dir_missing_backend_writes_backend_failure_contract() {
    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let manifest = dir.path().join("pipeline.yaml");
    let run_dir = dir.path().join("run");
    std::fs::write(&prompt, "do not leak this prompt").unwrap();
    write_file(
        &manifest,
        infer_manifest(prompt.display(), "missing:model", "answer.txt"),
    );

    llmff_cmd()
        .args([
            "run",
            "--run-dir",
            run_dir.to_str().unwrap(),
            manifest.to_str().unwrap(),
        ])
        .assert()
        .code(21)
        .stderr(predicate::str::contains("backend"));

    let result = read_run_result(&run_dir);
    assert_eq!(result["exit_code"], 21);
    assert_eq!(result["failure"]["kind"], "backend");
    assert_eq!(
        result["failure"]["retry_recommendation"],
        "retry_with_backoff"
    );

    assert!(!run_dir.join("events.jsonl").exists());
}

#[test]
fn run_dir_invalid_graph_writes_graph_validation_contract() {
    let dir = temp_dir();
    let manifest = dir.path().join("pipeline.yaml");
    let run_dir = dir.path().join("run");
    write_file(
        &manifest,
        r#"
version: 1
graph:
  - id: draft
    op: template
    from: missing_parent
    path: prompt.tmpl
outputs:
  final:
    from: draft
    path: answer.txt
"#,
    );

    llmff_cmd()
        .args([
            "run",
            "--run-dir",
            run_dir.to_str().unwrap(),
            manifest.to_str().unwrap(),
        ])
        .assert()
        .code(10)
        .stderr(predicate::str::contains("graph validation"));

    let result = read_run_result(&run_dir);
    assert_eq!(result["exit_code"], 10);
    assert_eq!(result["failure"]["kind"], "graph_validation");
    assert_eq!(
        result["failure"]["retry_recommendation"],
        "do_not_retry_without_changes"
    );
}

#[test]
fn run_dir_invalid_json_schema_writes_stage_failure_contract() {
    let dir = temp_dir();
    let prompt = dir.path().join("draft.json");
    let manifest = dir.path().join("pipeline.yaml");
    let run_dir = dir.path().join("run");
    std::fs::write(&prompt, r#"{"answer":"ok"}"#).unwrap();
    write_file(
        &manifest,
        format!(
            r#"
version: 1
inputs:
  draft:
    path: {}
    format: json
graph:
  - id: load_draft
    op: load
    input: draft
  - id: validate
    op: validate_json
    from: load_draft
    schema: "{{not valid json"
outputs:
  final:
    from: validate
    path: answer.json
"#,
            prompt.display()
        ),
    );

    llmff_cmd()
        .args([
            "run",
            "--run-dir",
            run_dir.to_str().unwrap(),
            manifest.to_str().unwrap(),
        ])
        .assert()
        .code(20)
        .stderr(predicate::str::contains("invalid inline schema"));

    let result = read_run_result(&run_dir);
    assert_eq!(result["exit_code"], 20);
    assert_eq!(result["failure"]["kind"], "stage_execution");
    assert_eq!(
        result["failure"]["retry_recommendation"],
        "check_stage_or_input"
    );
    let events = read_text_artifact(&run_dir, "events.jsonl");
    assert!(events.contains(r#""failure_kind":"stage_execution""#));
}

#[test]
fn run_dir_checkpoint_mismatch_writes_config_failure_contract() {
    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("answer.json");
    let checkpoint = dir.path().join("checkpoint.json");
    let manifest = dir.path().join("pipeline.yaml");
    let run_dir = dir.path().join("run");
    std::fs::write(&prompt, "Return an answer object").unwrap();
    write_file(
        &manifest,
        load_only_manifest(prompt.display(), output.display()),
    );

    llmff_cmd()
        .args([
            "run",
            "--checkpoint",
            checkpoint.to_str().unwrap(),
            manifest.to_str().unwrap(),
        ])
        .assert()
        .success();

    write_file(
        &manifest,
        load_only_manifest(prompt.display(), "changed-answer.json"),
    );

    llmff_cmd()
        .args([
            "run",
            "--run-dir",
            run_dir.to_str().unwrap(),
            "--resume",
            checkpoint.to_str().unwrap(),
            manifest.to_str().unwrap(),
        ])
        .assert()
        .code(10)
        .stderr(predicate::str::contains(
            "checkpoint manifest hash does not match current manifest",
        ))
        .stderr(predicate::str::contains("run inspect --format json"));

    let result = read_run_result(&run_dir);
    assert_eq!(result["exit_code"], 10);
    assert_eq!(result["failure"]["kind"], "config");
    assert_eq!(
        result["failure"]["retry_recommendation"],
        "do_not_retry_without_changes"
    );
    let events = read_text_artifact(&run_dir, "events.jsonl");
    assert!(events.contains(r#""failure_kind":"config""#));
}

#[test]
fn run_without_manifest_reports_usage_failure_contract() {
    llmff_cmd()
        .args(["run"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "provide either manifest or --graph",
        ));
}

#[test]
fn plugins_validate_json_invalid_plugin_reports_stable_failure_contract() {
    let dir = temp_dir();
    let plugin = dir.path().join("broken-plugin");
    std::fs::create_dir_all(&plugin).unwrap();
    write_file(
        plugin.join("llmff-plugin.yaml"),
        missing_backend_plugin_manifest(),
    );

    let output = llmff_cmd()
        .args([
            "plugins",
            "validate",
            "--plugin-dir",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("plugin validation failed"))
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value =
        serde_json::from_slice(&output).expect("plugin validation report should be JSON");
    assert_eq!(report["valid"], false);
    assert_eq!(report["diagnostics"][0]["code"], "missing_entrypoint");
}

#[test]
fn run_dir_timeout_writes_timeout_failure_contract() {
    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let manifest = dir.path().join("pipeline.yaml");
    let run_dir = dir.path().join("run");
    std::fs::write(&prompt, "slow input").unwrap();
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
  - id: slow_tool
    op: tool
    from: load_prompt
    command: ["/bin/sh", "-c", "sleep 1"]
    timeout_ms: 1
outputs:
  final:
    from: slow_tool
    path: answer.txt
"#,
            prompt.display()
        ),
    );

    llmff_cmd()
        .args([
            "run",
            "--run-dir",
            run_dir.to_str().unwrap(),
            manifest.to_str().unwrap(),
        ])
        .assert()
        .code(21)
        .stderr(predicate::str::contains("stage timed out"));

    let result = read_run_result(&run_dir);
    assert_eq!(result["exit_code"], 21);
    assert_eq!(result["failure"]["kind"], "timeout");
    assert_eq!(
        result["failure"]["retry_recommendation"],
        "check_stage_or_input"
    );
    let events = read_text_artifact(&run_dir, "events.jsonl");
    assert!(events.contains(r#""failure_kind":"timeout""#));
}

#[test]
fn run_accepts_parallel_scheduler_flag() {
    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("answer.json");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&prompt, "Return an answer object").unwrap();
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
  - id: draft_a
    op: infer
    from: load_prompt
    model: mock:good
  - id: draft_b
    op: infer
    from: load_prompt
    model: mock:good
outputs:
  final:
    from: draft_a
    path: {}
"#,
            prompt.display(),
            output.display()
        ),
    );

    let mut cmd = llmff_cmd();
    cmd.args(["run", "--parallel", manifest.to_str().unwrap()])
        .env("LLMFF_MOCK_GOOD_RESPONSE", r#"{"answer":"ok"}"#)
        .assert()
        .success();

    assert_eq!(read_file(output), r#"{"answer":"ok"}"#);
}

#[test]
fn run_accepts_execution_maturity_flags() {
    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("answer.json");
    let checkpoint = dir.path().join("checkpoint.json");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&prompt, "Return an answer object").unwrap();
    write_file(
        &manifest,
        infer_manifest(prompt.display(), "mock:good", output.display()),
    );

    llmff_cmd()
        .args([
            "run",
            "--parallel",
            "--max-concurrency",
            "1",
            "--timeout-ms",
            "1000",
            "--retry-attempts",
            "2",
            "--retry-backoff-ms",
            "0",
            "--checkpoint",
            checkpoint.to_str().unwrap(),
            manifest.to_str().unwrap(),
        ])
        .env("LLMFF_MOCK_GOOD_RESPONSE", r#"{"answer":"ok"}"#)
        .assert()
        .success();

    assert!(checkpoint.exists());
    assert_eq!(read_file(output), r#"{"answer":"ok"}"#);
}

#[test]
fn run_resume_reports_actionable_checkpoint_mismatch() {
    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("answer.json");
    let checkpoint = dir.path().join("checkpoint.json");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&prompt, "Return an answer object").unwrap();
    write_file(
        &manifest,
        load_only_manifest(prompt.display(), output.display()),
    );

    llmff_cmd()
        .args([
            "run",
            "--checkpoint",
            checkpoint.to_str().unwrap(),
            manifest.to_str().unwrap(),
        ])
        .assert()
        .success();

    write_file(
        &manifest,
        load_only_manifest(prompt.display(), "changed-answer.json"),
    );

    llmff_cmd()
        .args([
            "run",
            "--resume",
            checkpoint.to_str().unwrap(),
            manifest.to_str().unwrap(),
        ])
        .assert()
        .code(10)
        .stderr(predicate::str::contains(
            "checkpoint manifest hash does not match current manifest",
        ))
        .stderr(predicate::str::contains(checkpoint.to_str().unwrap()))
        .stderr(predicate::str::contains("checkpoint_hash="))
        .stderr(predicate::str::contains("current_manifest_hash="))
        .stderr(predicate::str::contains("run inspect --format json"));
}

#[test]
fn process_exit_codes_are_stable_for_supervisors() {
    llmff_cmd()
        .args(["inspect", "examples/json-repair.yaml"])
        .current_dir(workspace_root())
        .assert()
        .code(0);

    llmff_cmd()
        .args(["inspect", "-g", "load | write"])
        .assert()
        .code(10)
        .stderr(predicate::str::contains("graph validation"));

    llmff_cmd()
        .args(["run", "-g", "load | infer(model=missing:model) | write(-)"])
        .assert()
        .code(21)
        .stderr(predicate::str::contains("backend"));
}

#[cfg(unix)]
#[test]
fn interrupted_run_exits_with_stable_supervisor_code() {
    use std::os::unix::process::CommandExt;

    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let events = dir.path().join("events.jsonl");
    let output_path = dir.path().join("answer.txt");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&prompt, "do not leak this interrupted prompt").unwrap();
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
  - id: slow_tool
    op: tool
    from: load_prompt
    command: ["sh", "-c", "sleep 30"]
outputs:
  final:
    from: slow_tool
    path: {}
"#,
            prompt.display(),
            output_path.display()
        ),
    );

    let child = unsafe {
        let mut command = StdCommand::new(assert_cmd::cargo::cargo_bin("llmff"));
        command
            .args([
                "run",
                "--events",
                events.to_str().unwrap(),
                manifest.to_str().unwrap(),
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .pre_exec(|| {
                if libc::setpgid(0, 0) == 0 {
                    Ok(())
                } else {
                    Err(std::io::Error::last_os_error())
                }
            });
        command.spawn().unwrap()
    };

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if std::fs::read_to_string(&events)
            .map(|text| text.contains(r#""event":"stage_started""#))
            .unwrap_or(false)
        {
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }
    let event_text = std::fs::read_to_string(&events).unwrap_or_default();
    assert!(
        event_text.contains(r#""event":"stage_started""#),
        "run should start before interrupt, events were: {event_text}"
    );

    let pid = child.id() as libc::pid_t;
    let signal_result = unsafe { libc::kill(pid, libc::SIGINT) };
    assert_eq!(signal_result, 0, "SIGINT should reach llmff");

    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(130));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("interrupted"));

    let event_text = read_file(events);
    assert!(!event_text.contains("do not leak this interrupted prompt"));
}

#[cfg(unix)]
#[test]
fn run_dir_interrupted_run_writes_result_summary() {
    use std::os::unix::process::CommandExt;

    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let output_path = dir.path().join("answer.txt");
    let manifest = dir.path().join("pipeline.yaml");
    let run_dir = dir.path().join("run");
    std::fs::write(&prompt, "do not leak this interrupted prompt").unwrap();
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
  - id: slow_tool
    op: tool
    from: load_prompt
    command: ["sh", "-c", "sleep 30"]
outputs:
  final:
    from: slow_tool
    path: {}
"#,
            prompt.display(),
            output_path.display()
        ),
    );

    let child = unsafe {
        let mut command = StdCommand::new(assert_cmd::cargo::cargo_bin("llmff"));
        command
            .args([
                "run",
                "--run-dir",
                run_dir.to_str().unwrap(),
                manifest.to_str().unwrap(),
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .pre_exec(|| {
                if libc::setpgid(0, 0) == 0 {
                    Ok(())
                } else {
                    Err(std::io::Error::last_os_error())
                }
            });
        command.spawn().unwrap()
    };

    let events = run_dir.join("events.jsonl");
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if std::fs::read_to_string(&events)
            .map(|text| text.contains(r#""event":"stage_started""#))
            .unwrap_or(false)
        {
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }
    let event_text = std::fs::read_to_string(&events).unwrap_or_default();
    assert!(
        event_text.contains(r#""event":"stage_started""#),
        "run should start before interrupt, events were: {event_text}"
    );

    let pid = child.id() as libc::pid_t;
    let signal_result = unsafe { libc::kill(pid, libc::SIGINT) };
    assert_eq!(signal_result, 0, "SIGINT should reach llmff");

    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(130));
    assert!(output.stdout.is_empty());

    let result: serde_json::Value = read_run_result(&run_dir);
    assert_eq!(result["status"], "failed");
    assert_eq!(result["exit_code"], 130);
    assert_eq!(result["failure"]["kind"], "interrupted");
    assert_eq!(
        result["failure"]["retry_recommendation"],
        "resume_with_matching_checkpoint"
    );

    let event_text = read_file(events);
    assert!(event_text.contains(r#""event":"run_failed""#));
    assert!(event_text.contains(r#""failure_kind":"interrupted""#));
    assert!(!event_text.contains("do not leak this interrupted prompt"));
}

#[cfg(unix)]
#[test]
fn run_dir_interrupted_run_preserves_completed_checkpoint() {
    use std::os::unix::process::CommandExt;

    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let output_path = dir.path().join("answer.txt");
    let manifest = dir.path().join("pipeline.yaml");
    let run_dir = dir.path().join("run");
    std::fs::write(&prompt, "checkpoint-safe prompt").unwrap();
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
  - id: slow_tool
    op: tool
    from: load_prompt
    command: ["sh", "-c", "sleep 30"]
outputs:
  final:
    from: slow_tool
    path: {}
"#,
            prompt.display(),
            output_path.display()
        ),
    );

    let child = unsafe {
        let mut command = StdCommand::new(assert_cmd::cargo::cargo_bin("llmff"));
        command
            .args([
                "run",
                "--run-dir",
                run_dir.to_str().unwrap(),
                manifest.to_str().unwrap(),
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .pre_exec(|| {
                if libc::setpgid(0, 0) == 0 {
                    Ok(())
                } else {
                    Err(std::io::Error::last_os_error())
                }
            });
        command.spawn().unwrap()
    };

    let checkpoint = run_dir.join("checkpoint.json");
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if std::fs::read_to_string(&checkpoint)
            .map(|text| text.contains(r#""load_prompt""#) && !text.contains(r#""slow_tool""#))
            .unwrap_or(false)
        {
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }
    let checkpoint_text = std::fs::read_to_string(&checkpoint).unwrap_or_default();
    assert!(
        checkpoint_text.contains(r#""load_prompt""#),
        "completed stage should be checkpointed: {checkpoint_text}"
    );
    assert!(
        !checkpoint_text.contains(r#""slow_tool""#),
        "interrupted stage should not be checkpointed: {checkpoint_text}"
    );

    let pid = child.id() as libc::pid_t;
    let signal_result = unsafe { libc::kill(pid, libc::SIGINT) };
    assert_eq!(signal_result, 0, "SIGINT should reach llmff");

    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(130));

    let checkpoint_text = read_file(checkpoint);
    assert!(checkpoint_text.contains(r#""load_prompt""#));
    assert!(!checkpoint_text.contains(r#""slow_tool""#));
}

#[test]
fn process_exit_code_reports_stage_execution_failure() {
    let dir = temp_dir();
    let tool = dir.path().join("fail-tool");
    write_file(
        &tool,
        r#"#!/bin/sh
cat >/dev/null
printf 'tool failed\n' >&2
exit 7
"#,
    );
    make_executable(&tool);

    llmff_cmd()
        .args([
            "run",
            "-g",
            &format!("load | tool(command={}) | write(-)", tool.to_string_lossy()),
        ])
        .write_stdin("payload")
        .assert()
        .code(20)
        .stderr(predicate::str::contains("tool command exited with status"));
}
