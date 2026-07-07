mod common;

use common::*;
use predicates::prelude::*;

#[test]
fn inspect_example_manifest_succeeds() {
    let mut cmd = llmff_cmd();

    cmd.args(["inspect", "examples/json-repair.yaml"])
        .current_dir(workspace_root())
        .assert()
        .success()
        .stdout(predicate::str::contains("ok"));
}

#[test]
fn inspect_json_reports_reproducible_execution_contract() {
    let root = workspace_root();
    let output = llmff_cmd()
        .args([
            "inspect",
            "examples/json-repair.yaml",
            "--format",
            "json",
            "--plugin-dir",
            "examples/plugins",
            "--backend",
            "gateway=https://gateway.example/v1",
            "--ollama",
            "local=http://localhost:11434",
        ])
        .current_dir(&root)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let report: serde_json::Value =
        serde_json::from_slice(&output).expect("inspect report should be valid JSON");

    assert_eq!(report["format_version"], 1);
    assert_eq!(report["manifest"]["version"], 1);
    assert_eq!(report["manifest"]["source"]["kind"], "file");
    assert_eq!(
        report["manifest"]["source"]["path"],
        "examples/json-repair.yaml"
    );
    assert!(report["manifest"]["hash"]
        .as_str()
        .expect("manifest hash should be a string")
        .starts_with("sha256:"));
    assert_eq!(report["compatibility"]["pipeline_manifest_schema"], 1);
    assert_eq!(report["compatibility"]["inspect_report_schema"], 1);
    assert_eq!(report["compatibility"]["inline_graph_syntax"], 1);
    assert_eq!(report["compatibility"]["plugin_protocol"], 1);

    assert_eq!(report["inputs"]["prompt"]["path"], "./question.txt");
    assert_eq!(report["outputs"]["final"]["from"], "choose_final");
    assert_eq!(report["outputs"]["final"]["path"], "./answer.json");
    assert_eq!(
        report["stage_order"],
        serde_json::json!([
            "load_prompt",
            "render_prompt",
            "apply_policy",
            "draft",
            "validate",
            "repair",
            "choose_final"
        ])
    );
    assert_eq!(report["stages"][3]["id"], "draft");
    assert_eq!(report["stages"][3]["model"]["alias"], "mock");
    assert_eq!(report["stages"][3]["model"]["provider_model"], "bad");
    assert_eq!(
        report["stages"][3]["capability_constraints"]["kind"],
        "model"
    );
    assert_eq!(
        report["stages"][3]["capability_constraints"]["required_fields"],
        serde_json::json!(["from", "model"])
    );
    assert!(
        report["stages"][3]["capability_constraints"]["capabilities"]
            .as_array()
            .expect("capabilities should be an array")
            .contains(&serde_json::json!("response-format-json"))
    );
    assert_eq!(
        report["stages"][4]["capability_constraints"]["required_fields"],
        serde_json::json!(["from", "schema|schema_path"])
    );
    assert!(
        report["stages"][6]["capability_constraints"]["capabilities"]
            .as_array()
            .expect("route capabilities should be an array")
            .contains(&serde_json::json!("status-routing"))
    );
    assert_eq!(report["execution"]["scheduler"], "sequential");
    assert_eq!(report["execution"]["stdout"]["events"], false);
    assert_eq!(report["execution"]["stdout"]["stream_stage"], false);
    assert_eq!(report["backends"]["registrations"][0]["name"], "mock");
    assert_eq!(report["backends"]["registrations"][0]["source"], "built-in");
    assert_eq!(report["backends"]["registrations"][3]["name"], "gateway");
    assert_eq!(report["backends"]["registrations"][3]["source"], "cli");
    assert_eq!(
        report["backends"]["registrations"][3]["base_url"],
        "https://gateway.example/v1"
    );
    assert_eq!(report["backends"]["registrations"][4]["name"], "local");
    assert_eq!(report["backends"]["registrations"][4]["kind"], "ollama");
    assert_eq!(
        report["backends"]["registrations"][5]["kind"],
        "plugin-command"
    );
    assert_eq!(
        report["plugins"]["directories"],
        serde_json::json!(["examples/plugins"])
    );
    assert_eq!(report["plugins"]["protocol_version"], 1);
    assert_eq!(report["plugins"]["manifests"][0]["name"], "backend-echo");
    assert_eq!(
        report["plugins"]["manifests"][0]["capabilities"][0]["kind"],
        "backend"
    );
}

#[test]
fn inspect_json_reports_loop_expansion_bound() {
    let dir = temp_dir();
    let manifest = dir.path().join("loop.yaml");
    write_file(
        &manifest,
        r#"
version: 1
inputs:
  prompt:
    path: prompt.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: refine
    op: loop
    from: load_prompt
    max_iterations: 3
    break_on: { type: never }
    final: { from: draft, require_status: success }
    body:
      - id: draft
        op: infer
        from: input
        model: mock:json
"#,
    );
    std::fs::write(dir.path().join("prompt.txt"), "question").unwrap();

    let output = llmff_cmd()
        .current_dir(dir.path())
        .args(["inspect", manifest.to_str().unwrap(), "--format", "json"])
        .output()
        .expect("inspect should run");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let refine = report["stages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|stage| stage["id"] == "refine")
        .expect("refine stage should be reported");
    assert_eq!(refine["loop"]["max_iterations"], 3);
    assert_eq!(refine["loop"]["body_stage_count"], 1);
    assert_eq!(refine["loop"]["max_expanded_stage_count"], 3);
    assert_eq!(refine["loop"]["break_on"]["type"], "never");
    assert_eq!(refine["loop"]["final"]["from"], "draft");
    assert_eq!(refine["loop"]["final"]["require_status"], "success");
    assert_eq!(refine["loop"]["retain_iterations"], "none");
    assert_eq!(refine["loop"]["on_iteration_error"], "fail");
}

#[test]
fn inspect_json_reports_requested_execution_options() {
    let root = workspace_root();
    let checkpoint = root.join("target/inspect-checkpoint.json");
    let output = llmff_cmd()
        .args([
            "inspect",
            "examples/json-repair.yaml",
            "--format",
            "json",
            "--parallel",
            "--max-concurrency",
            "4",
            "--timeout-ms",
            "30000",
            "--retry-attempts",
            "3",
            "--retry-backoff-ms",
            "250",
            "--checkpoint",
            checkpoint.to_str().unwrap(),
            "--resume",
            checkpoint.to_str().unwrap(),
            "--events",
            "-",
            "--trace",
            "target/inspect-trace.jsonl",
        ])
        .current_dir(&root)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let report: serde_json::Value =
        serde_json::from_slice(&output).expect("inspect report should be valid JSON");

    assert_eq!(report["execution"]["scheduler"], "parallel");
    assert_eq!(report["execution"]["max_concurrency"], 4);
    assert_eq!(report["execution"]["default_timeout_ms"], 30000);
    assert_eq!(report["execution"]["default_retry"]["attempts"], 3);
    assert_eq!(report["execution"]["default_retry"]["backoff_ms"], 250);
    assert_eq!(report["execution"]["checkpoint"]["enabled"], true);
    assert_eq!(report["execution"]["checkpoint"]["resume"], true);
    assert_eq!(
        report["execution"]["checkpoint"]["path"],
        checkpoint.to_string_lossy().as_ref()
    );
    assert_eq!(
        report["execution"]["checkpoint"]["resume_path"],
        checkpoint.to_string_lossy().as_ref()
    );
    assert_eq!(report["execution"]["stdout"]["events"], true);
    assert_eq!(report["execution"]["stdout"]["stream_stage"], false);
    assert_eq!(
        report["execution"]["artifacts"]["trace"],
        "target/inspect-trace.jsonl"
    );
}

#[test]
fn inspect_rejects_requested_stdout_conflicts() {
    let manifest = tempfile::NamedTempFile::new().unwrap();
    write_file(
        manifest.path(),
        r#"
version: 1
inputs:
  prompt:
    path: "-"
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: write_answer
    op: write
    from: load_prompt
    path: "-"
outputs:
  final:
    from: write_answer
    path: "-"
"#,
    );

    llmff_cmd()
        .args([
            "inspect",
            manifest.path().to_str().unwrap(),
            "--format",
            "json",
            "--events",
            "-",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "events cannot stream to stdout while manifest outputs write to stdout",
        ));
}

#[test]
fn inspect_accepts_plugin_stage_with_plugin_dir() {
    let dir = temp_dir();
    let plugin_dir = dir.path().join("plugins");
    let plugin = plugin_dir.join("text-plugin");
    std::fs::create_dir_all(&plugin).unwrap();
    write_file(
        plugin.join("llmff-plugin.yaml"),
        r#"
name: text-plugin
version: 0.1.0
capabilities:
  - kind: stage
    name: text.uppercase
    entrypoint: /bin/cat
"#,
    );
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("stage-output.txt");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&prompt, "plugin stage").unwrap();
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
  - id: uppercase
    op: plugin:text.uppercase
    from: load_prompt
outputs:
  final:
    from: uppercase
    path: {}
"#,
            prompt.display(),
            output.display()
        ),
    );

    let mut cmd = llmff_cmd();
    cmd.args([
        "inspect",
        manifest.to_str().unwrap(),
        "--plugin-dir",
        plugin_dir.to_str().unwrap(),
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("ok"));
}

#[test]
fn inspect_accepts_plugin_backend_with_plugin_dir() {
    let dir = temp_dir();
    let plugin_dir = dir.path().join("plugins");
    let plugin = plugin_dir.join("model-plugin");
    std::fs::create_dir_all(&plugin).unwrap();
    write_file(
        plugin.join("llmff-plugin.yaml"),
        local_echo_model_plugin_manifest(),
    );
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("answer.txt");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&prompt, "ask plugin backend").unwrap();
    write_file(
        &manifest,
        infer_manifest(prompt.display(), "local-echo:test-model", output.display()),
    );

    let mut cmd = llmff_cmd();
    cmd.args([
        "inspect",
        manifest.to_str().unwrap(),
        "--plugin-dir",
        plugin_dir.to_str().unwrap(),
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("ok"));
}

#[test]
fn inspect_accepts_plugin_sampler_with_plugin_dir() {
    let dir = temp_dir();
    let plugin_dir = dir.path().join("plugins");
    let plugin = plugin_dir.join("sampling-plugin");
    std::fs::create_dir_all(&plugin).unwrap();
    write_file(
        plugin.join("llmff-plugin.yaml"),
        r#"
name: sampling-plugin
version: 0.1.0
capabilities:
  - kind: sampler
    name: safe-small
    entrypoint: /bin/false
"#,
    );
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("answer.txt");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&prompt, "ask sampled backend").unwrap();
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
    model: mock:good
    sampler: safe-small
outputs:
  final:
    from: draft
    path: {}
"#,
            prompt.display(),
            output.display()
        ),
    );

    let mut cmd = llmff_cmd();
    cmd.args([
        "inspect",
        manifest.to_str().unwrap(),
        "--plugin-dir",
        plugin_dir.to_str().unwrap(),
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("ok"));
}

#[test]
fn inspect_accepts_inline_graph() {
    let mut cmd = llmff_cmd();

    cmd.args(["inspect", "-g", "load | infer(model=mock:good) | write(-)"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ok"));
}

#[test]
fn inspect_rejects_inline_graph_with_missing_backend() {
    let mut cmd = llmff_cmd();

    cmd.args([
        "inspect",
        "-g",
        "load | infer(model=openai:gpt-test) | write(-)",
    ])
    .assert()
    .failure()
    .stderr(predicate::str::contains(
        "no backend configured for `openai:gpt-test`",
    ));
}

#[test]
fn inspect_rejects_unregistered_backend_alias() {
    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
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
    model: openai:gpt-test
"#,
            prompt.display()
        ),
    );

    let mut cmd = llmff_cmd();
    cmd.args(["inspect", manifest.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "no backend configured for `openai:gpt-test`",
        ));
}

#[test]
fn inspect_accepts_registered_openai_backend_without_calling_server() {
    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
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
    model: openai:gpt-test
"#,
            prompt.display()
        ),
    );

    let mut cmd = llmff_cmd();
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
    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let template = dir.path().join("fast.tmpl");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&prompt, r#"{"kind":"simple"}"#).unwrap();
    std::fs::write(&template, "fast").unwrap();
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
    );

    let mut cmd = llmff_cmd();
    cmd.args(["inspect", manifest.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "field route requires JSON source `load_prompt`, got text",
        ));
}

#[test]
fn inspect_accepts_field_route_from_json_input() {
    let dir = temp_dir();
    let payload = dir.path().join("payload.json");
    let template = dir.path().join("simple.tmpl");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&payload, r#"{"kind":"simple","answer":"ok"}"#).unwrap();
    std::fs::write(&template, "{{answer}}").unwrap();
    write_file(
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
    );

    let mut cmd = llmff_cmd();
    cmd.args(["inspect", manifest.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("ok"));
}

#[test]
fn inspect_rejects_unknown_when_condition() {
    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
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
    when: maybe
    model: mock:good
"#,
            prompt.display()
        ),
    );

    let mut cmd = llmff_cmd();
    cmd.args(["inspect", manifest.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown when condition `maybe`"));
}

#[test]
fn inspect_rejects_unknown_input_format() {
    let dir = temp_dir();
    let payload = dir.path().join("payload.json");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&payload, r#"{"kind":"simple"}"#).unwrap();
    write_file(
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
    );

    let mut cmd = llmff_cmd();
    cmd.args(["inspect", manifest.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "input `payload` has unsupported format `yaml`",
        ));
}

#[test]
fn inspect_rejects_invalid_sampling_parameters() {
    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
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
    model: mock:good
    max_tokens: 0
"#,
            prompt.display()
        ),
    );

    let mut cmd = llmff_cmd();
    cmd.args(["inspect", manifest.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "max_tokens must be greater than 0",
        ));
}

#[test]
fn inspect_accepts_inline_graph_stop_sequences() {
    let prompt = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(prompt.path(), "Return an answer object").unwrap();

    let mut cmd = llmff_cmd();
    cmd.args([
        "inspect",
        "-i",
        prompt.path().to_str().unwrap(),
        "-g",
        "load | infer(model=mock:good,stop=END;DONE) | write(-)",
    ])
    .assert()
    .success();
}

#[test]
fn inspect_accepts_inline_graph_json_response_format() {
    let prompt = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(prompt.path(), "Return an answer object").unwrap();

    let mut cmd = llmff_cmd();
    cmd.args([
        "inspect",
        "-i",
        prompt.path().to_str().unwrap(),
        "-g",
        "load | infer(model=mock:good,response_format=json) | write(-)",
    ])
    .assert()
    .success();
}

#[test]
fn inspect_accepts_inline_graph_seed() {
    let prompt = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(prompt.path(), "Return an answer object").unwrap();

    let mut cmd = llmff_cmd();
    cmd.args([
        "inspect",
        "-i",
        prompt.path().to_str().unwrap(),
        "-g",
        "load | infer(model=mock:good,seed=12345) | write(-)",
    ])
    .assert()
    .success();
}
