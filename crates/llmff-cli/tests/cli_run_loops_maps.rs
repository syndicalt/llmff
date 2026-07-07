mod common;

use common::*;
use predicates::prelude::*;

#[test]
fn run_executes_retrieve_stage() {
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
    cmd.args(["run", manifest.to_str().unwrap()])
        .assert()
        .success();

    let json: serde_json::Value =
        serde_json::from_str(&read_file(output)).expect("retrieve output should be JSON");
    assert_eq!(json["query"], "rust graph");
    assert_eq!(json["matches"].as_array().unwrap().len(), 1);
    assert_eq!(json["matches"][0]["path"], "docs/rust.txt");
    assert_eq!(json["matches"][0]["score"], 2);
}

#[test]
fn run_executes_cache_stage() {
    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("answer.txt");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&prompt, "first").unwrap();
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
  - id: cached
    op: cache
    from: load_prompt
    path: .llmff/cache
    key: answer-v1
outputs:
  final:
    from: cached
    path: {}
"#,
            prompt.display(),
            output.display()
        ),
    );

    let mut first = llmff_cmd();
    first
        .args(["run", manifest.to_str().unwrap()])
        .assert()
        .success();
    assert_eq!(read_file(&output), "first");

    std::fs::write(&prompt, "second").unwrap();
    let mut second = llmff_cmd();
    second
        .args(["run", manifest.to_str().unwrap()])
        .assert()
        .success();
    assert_eq!(read_file(&output), "first");
}

#[test]
fn run_supports_stdin_and_stdout_paths() {
    let dir = temp_dir();
    let manifest = dir.path().join("pipeline.yaml");
    write_file(
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
    );

    let mut cmd = llmff_cmd();
    cmd.args(["run", manifest.to_str().unwrap()])
        .write_stdin("Return JSON")
        .env("LLMFF_MOCK_GOOD_RESPONSE", r#"{"answer":"ok"}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#"{"answer":"ok"}"#));
}

#[test]
fn run_reports_invalid_json_input_format() {
    let dir = temp_dir();
    let payload = dir.path().join("payload.json");
    let output = dir.path().join("selected.txt");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&payload, "{not-json").unwrap();
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
outputs:
  final:
    from: load_payload
    path: {}
"#,
            payload.display(),
            output.display()
        ),
    );

    let mut cmd = llmff_cmd();
    cmd.args(["run", manifest.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "input `payload` is not valid JSON",
        ));
}

#[test]
fn run_routes_json_input_by_field() {
    let dir = temp_dir();
    let payload = dir.path().join("payload.json");
    let template = dir.path().join("simple.tmpl");
    let output = dir.path().join("selected.txt");
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
outputs:
  final:
    from: choose
    path: {}
"#,
            payload.display(),
            template.display(),
            output.display()
        ),
    );

    let mut cmd = llmff_cmd();
    cmd.args(["run", manifest.to_str().unwrap()])
        .assert()
        .success();

    assert_eq!(read_file(output), "ok");
}

#[test]
fn run_extracts_nested_json_field() {
    let dir = temp_dir();
    let payload = dir.path().join("payload.json");
    let output = dir.path().join("selected.json");
    let manifest = dir.path().join("pipeline.yaml");
    write_file(
        &payload,
        r#"{"result":{"final_answer":{"answer":"ok","score":9}}}"#,
    );
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
  - id: selected
    op: extract
    from: load_payload
    json_path: result.final_answer
outputs:
  final:
    from: selected
    path: {}
"#,
            payload.display(),
            output.display()
        ),
    );

    let mut cmd = llmff_cmd();
    cmd.args(["run", manifest.to_str().unwrap()])
        .assert()
        .success();

    let written: serde_json::Value = serde_json::from_str(&read_file(output)).unwrap();
    assert_eq!(written, serde_json::json!({"answer": "ok", "score": 9}));
}

#[test]
fn run_accumulates_with_state_from() {
    let dir = temp_dir();
    let previous = dir.path().join("previous.json");
    let current = dir.path().join("current.json");
    let output = dir.path().join("history.json");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&previous, r#"[{"id":"a","value":1},{"id":"b","value":2}]"#).unwrap();
    std::fs::write(&current, r#"{"id":"a","value":3}"#).unwrap();
    write_file(
        &manifest,
        format!(
            r#"
version: 1
inputs:
  previous:
    path: {}
    format: json
  current:
    path: {}
    format: json
graph:
  - id: load_previous
    op: load
    input: previous
  - id: load_current
    op: load
    input: current
  - id: history
    op: accumulate
    from: load_current
    state_from: load_previous
    mode: append
    limit: 2
    dedupe_field: id
outputs:
  final:
    from: history
    path: {}
"#,
            previous.display(),
            current.display(),
            output.display()
        ),
    );

    let mut cmd = llmff_cmd();
    cmd.args(["run", manifest.to_str().unwrap()])
        .assert()
        .success();

    let written: serde_json::Value = serde_json::from_str(&read_file(output)).unwrap();
    assert_eq!(
        written,
        serde_json::json!([{"id": "b", "value": 2}, {"id": "a", "value": 3}])
    );
}

#[test]
fn run_loop_retains_iteration_values() {
    let dir = temp_dir();
    let input = dir.path().join("input.json");
    let output = dir.path().join("loop.json");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&input, r#"{"value":"kept"}"#).unwrap();
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
  - id: keep
    op: loop
    from: load_payload
    max_iterations: 2
    break_on: {{ type: never }}
    retain_iterations:
      mode: all
      stages: [current]
      include_values: true
    final: {{ from: current, require_status: success }}
    body:
      - id: current
        op: extract
        from: input
        field: value
outputs:
  final:
    from: keep
    path: {}
"#,
            input.display(),
            output.display()
        ),
    );

    let mut cmd = llmff_cmd();
    cmd.args(["run", manifest.to_str().unwrap()])
        .assert()
        .success();

    let written: serde_json::Value = serde_json::from_str(&read_file(output)).unwrap();
    assert_eq!(written["metadata"]["iterations_run"], 2);
    assert_eq!(written["iterations"].as_array().unwrap().len(), 2);
    assert_eq!(
        written["iterations"][0]["stages"]["current"]["status"],
        "success"
    );
    assert_eq!(
        written["iterations"][0]["stages"]["current"]["value"],
        "kept"
    );
}

#[test]
fn run_loop_initial_carry_seeds_accumulator() {
    let dir = temp_dir();
    let input = dir.path().join("input.json");
    let output = dir.path().join("loop.json");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&input, r#"{"id":"a","value":1}"#).unwrap();
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
  - id: collect
    op: loop
    from: load_payload
    max_iterations: 2
    break_on: {{ type: never }}
    initial_carry:
      history: []
    carry:
      history: updated_history
    final: {{ from: updated_history, require_status: success }}
    body:
      - id: updated_history
        op: accumulate
        from: input
        state_from: history
        mode: append
outputs:
  final:
    from: collect
    path: {}
"#,
            input.display(),
            output.display()
        ),
    );

    let mut cmd = llmff_cmd();
    cmd.args(["run", manifest.to_str().unwrap()])
        .assert()
        .success();

    let written: serde_json::Value = serde_json::from_str(&read_file(output)).unwrap();
    assert_eq!(
        written["final"],
        serde_json::json!([
            {"id": "a", "value": 1},
            {"id": "a", "value": 1}
        ])
    );
}

#[test]
fn run_scores_json_payload() {
    let dir = temp_dir();
    let input = dir.path().join("judge.json");
    let output = dir.path().join("score.json");
    let manifest = dir.path().join("pipeline.yaml");
    write_file(
        &input,
        r#"{"result":{"score":8,"reason":"cited","label":"usable"}}"#,
    );
    write_file(
        &manifest,
        format!(
            r#"
version: 1
inputs:
  judge:
    path: {}
    format: json
graph:
  - id: load_judge
    op: load
    input: judge
  - id: normalized
    op: score
    from: load_judge
    score_field: result.score
    reason_field: result.reason
    label_field: result.label
    min_score: 0
    max_score: 10
outputs:
  final:
    from: normalized
    path: {}
"#,
            input.display(),
            output.display()
        ),
    );

    let mut cmd = llmff_cmd();
    cmd.args(["run", manifest.to_str().unwrap()])
        .assert()
        .success();

    let written: serde_json::Value = serde_json::from_str(&read_file(output)).unwrap();
    assert_eq!(written["score"], 8.0);
    assert_eq!(written["reason"], "cited");
    assert_eq!(written["label"], "usable");
}

#[test]
fn run_selects_highest_scored_candidate() {
    let dir = temp_dir();
    let input = dir.path().join("candidates.json");
    let output = dir.path().join("winner.json");
    let manifest = dir.path().join("pipeline.yaml");
    write_file(
        &input,
        r#"[{"answer":"a","score":7},{"answer":"b","score":9},{"answer":"c","score":9}]"#,
    );
    write_file(
        &manifest,
        format!(
            r#"
version: 1
inputs:
  candidates:
    path: {}
    format: json
graph:
  - id: load_candidates
    op: load
    input: candidates
  - id: winner
    op: select
    from: load_candidates
    mode: highest_score
    score_field: score
outputs:
  final:
    from: winner
    path: {}
"#,
            input.display(),
            output.display()
        ),
    );

    let mut cmd = llmff_cmd();
    cmd.args(["run", manifest.to_str().unwrap()])
        .assert()
        .success();

    let written: serde_json::Value = serde_json::from_str(&read_file(output)).unwrap();
    assert_eq!(written["selected"]["answer"], "b");
    assert_eq!(written["metadata"]["selected_index"], 1);
}

#[test]
fn run_maps_items_sequentially_with_max_items_cap() {
    let dir = temp_dir();
    let input = dir.path().join("items.json");
    let output = dir.path().join("mapped.json");
    let manifest = dir.path().join("pipeline.yaml");
    write_file(
        &input,
        r#"{"items":[{"name":"a"},{"name":"b"},{"name":"c"}]}"#,
    );
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
  - id: names
    op: map
    from: load_payload
    items_from: items
    max_items: 2
    final: {{ from: name, require_status: success }}
    body:
      - id: name
        op: extract
        from: item
        field: name
outputs:
  final:
    from: names
    path: {}
"#,
            input.display(),
            output.display()
        ),
    );

    let mut cmd = llmff_cmd();
    cmd.args(["run", manifest.to_str().unwrap()])
        .assert()
        .success();

    let written: serde_json::Value = serde_json::from_str(&read_file(output)).unwrap();
    assert_eq!(written["metadata"]["items_total"], 3);
    assert_eq!(written["metadata"]["items_run"], 2);
    assert_eq!(written["metadata"]["stop_reason"], "max_items");
    assert_eq!(written["items"][0]["index"], 0);
    assert_eq!(written["items"][0]["status"], "success");
    assert_eq!(written["items"][0]["value"], "a");
    assert_eq!(written["items"][1]["value"], "b");
}
