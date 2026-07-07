mod common;

use common::*;
use predicates::prelude::*;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[test]
fn inline_graph_run_uses_input_graph_and_write_stage() {
    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("answer.json");
    std::fs::write(&prompt, "Return an answer object").unwrap();

    let mut cmd = llmff_cmd();
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

    assert_eq!(read_file(output), r#"{"answer":"ok"}"#);
}

#[test]
fn inline_graph_run_defaults_load_to_stdin_and_write_to_stdout() {
    let mut cmd = llmff_cmd();
    cmd.args(["run", "-g", "load | infer(model=mock:good) | write(-)"])
        .write_stdin("Return an answer object")
        .env("LLMFF_MOCK_GOOD_RESPONSE", r#"{"answer":"ok"}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#"{"answer":"ok"}"#));
}

#[test]
fn inline_graph_run_executes_retrieve_stage() {
    let dir = temp_dir();
    let docs = dir.path().join("docs");
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("matches.json");
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

    let mut cmd = llmff_cmd();
    cmd.current_dir(dir.path())
        .args([
            "run",
            "-i",
            prompt.to_str().unwrap(),
            "-g",
            "load | retrieve(documents=docs/python.txt;docs/rust.txt,top_k=1) | write(matches.json)",
        ])
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
fn inline_graph_run_executes_named_from_references() {
    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let template = dir.path().join("prompt.tmpl");
    let output = dir.path().join("answer.txt");
    std::fs::write(&prompt, "graph").unwrap();
    std::fs::write(&template, "Question: {{ input }}").unwrap();

    let mut cmd = llmff_cmd();
    cmd.current_dir(dir.path())
        .args([
            "run",
            "-i",
            prompt.to_str().unwrap(),
            "-g",
            "load#prompt | template#render(prompt.tmpl) | infer#draft(from=render,model=mock:good) | write#save(from=draft,path=answer.txt)",
        ])
        .env("LLMFF_MOCK_GOOD_RESPONSE", "named graph ok")
        .assert()
        .success();

    assert_eq!(read_file(output), "named graph ok");
}

#[test]
fn inline_graph_run_executes_embedding_retrieve_strategy() {
    let dir = temp_dir();
    let docs = dir.path().join("docs");
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("matches.json");
    std::fs::create_dir(&docs).unwrap();
    std::fs::write(&prompt, "rust").unwrap();
    std::fs::write(docs.join("trust.txt"), "Trust systems keep state.").unwrap();
    std::fs::write(docs.join("python.txt"), "Python notebooks handle tables.").unwrap();

    let mut cmd = llmff_cmd();
    cmd.current_dir(dir.path())
        .args([
            "run",
            "-i",
            prompt.to_str().unwrap(),
            "-g",
            "load | retrieve(documents=docs/python.txt;docs/trust.txt,top_k=1,strategy=embedding) | write(matches.json)",
        ])
        .assert()
        .success();

    let json: serde_json::Value =
        serde_json::from_str(&read_file(output)).expect("retrieve output should be JSON");

    assert_eq!(json["strategy"], "embedding");
    assert_eq!(json["matches"][0]["path"], "docs/trust.txt");
    assert!(json["matches"][0]["score"].as_f64().unwrap() > 0.0);
}

#[test]
fn inline_graph_run_reuses_persistent_embedding_retrieve_index() {
    let dir = temp_dir();
    let docs = dir.path().join("docs");
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("matches.json");
    let index = dir.path().join(".llmff/retrieve/context.index.json");
    std::fs::create_dir(&docs).unwrap();
    std::fs::write(&prompt, "rust").unwrap();
    std::fs::write(docs.join("trust.txt"), "Trust systems keep state.").unwrap();
    std::fs::write(docs.join("python.txt"), "Python notebooks handle tables.").unwrap();

    for _ in 0..2 {
        let mut cmd = llmff_cmd();
        cmd.current_dir(dir.path())
            .args([
                "run",
                "-i",
                prompt.to_str().unwrap(),
                "-g",
                "load | retrieve(documents=docs/python.txt;docs/trust.txt,top_k=1,strategy=embedding,index=.llmff/retrieve/context.index.json) | write(matches.json)",
            ])
            .assert()
            .success();
    }

    let json: serde_json::Value =
        serde_json::from_str(&read_file(output)).expect("retrieve output should be JSON");

    assert!(index.exists());
    assert_eq!(json["strategy"], "embedding");
    assert_eq!(json["index"]["path"], ".llmff/retrieve/context.index.json");
    assert_eq!(json["index"]["reused_documents"], 2);
    assert_eq!(json["index"]["indexed_documents"], 0);
    assert_eq!(json["matches"][0]["path"], "docs/trust.txt");
}

#[test]
fn run_executes_rerank_stage() {
    let dir = temp_dir();
    let candidates = dir.path().join("matches.json");
    let manifest = dir.path().join("pipeline.yaml");
    let output = dir.path().join("reranked.json");
    write_file(
        &candidates,
        r#"
{
  "query": "rust",
  "strategy": "lexical",
  "matches": [
    {
      "path": "docs/python.txt",
      "score": 1,
      "text": "Python notebooks handle tables."
    },
    {
      "path": "docs/trust.txt",
      "score": 0,
      "text": "Trust systems keep state."
    }
  ]
}
"#,
    );
    write_file(
        &manifest,
        r#"
version: 1
inputs:
  candidates:
    path: matches.json
    format: json
graph:
  - id: load_candidates
    op: load
    input: candidates
  - id: rerank_context
    op: rerank
    from: load_candidates
    strategy: embedding
    top_k: 1
  - id: write_matches
    op: write
    from: rerank_context
    path: reranked.json
"#,
    );

    let mut cmd = llmff_cmd();
    cmd.current_dir(dir.path())
        .args(["run", manifest.to_str().unwrap()])
        .assert()
        .success();

    let json: serde_json::Value =
        serde_json::from_str(&read_file(output)).expect("rerank output should be JSON");

    assert_eq!(json["strategy"], "embedding");
    assert_eq!(json["matches"].as_array().unwrap().len(), 1);
    assert_eq!(json["matches"][0]["path"], "docs/trust.txt");
    assert!(json["matches"][0]["score"].as_f64().unwrap() > 0.0);
}

#[test]
fn run_executes_command_retrieve_strategy() {
    let dir = temp_dir();
    let docs = dir.path().join("docs");
    let bin = dir.path().join("bin");
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("matches.json");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::create_dir(&docs).unwrap();
    std::fs::create_dir(&bin).unwrap();
    std::fs::write(&prompt, "rust graph").unwrap();
    write_file(
        docs.join("rust.txt"),
        "Rust builds reliable graph pipelines.",
    );
    let command = bin.join("retrieve");
    write_file(
        &command,
        r#"#!/bin/sh
request=$(cat)
case "$request" in
  *'"query":"rust graph"'*)
    case "$request" in
      *'"path":"docs/rust.txt"'*)
        printf '{"query":"rust graph","strategy":"command","matches":[{"path":"remote://rust","score":0.99,"text":"remote result"}]}'
        ;;
      *)
        printf '%s\n' "$request" >&2
        exit 8
        ;;
    esac
    ;;
  *)
    printf '%s\n' "$request" >&2
    exit 8
    ;;
esac
"#,
    );
    make_executable(&command);
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
    strategy: command
    command: [{}]
    documents:
      - docs/rust.txt
    top_k: 1
outputs:
  final:
    from: retrieve_context
    path: {}
"#,
            prompt.display(),
            command.display(),
            output.display()
        ),
    );

    let mut cmd = llmff_cmd();
    cmd.current_dir(dir.path())
        .args(["run", manifest.to_str().unwrap()])
        .assert()
        .success();

    let json: serde_json::Value =
        serde_json::from_str(&read_file(output)).expect("command retrieve output should be JSON");
    assert_eq!(json["strategy"], "command");
    assert_eq!(json["matches"][0]["path"], "remote://rust");
    assert_eq!(json["matches"][0]["score"], 0.99);
}

#[test]
fn run_executes_command_rerank_strategy() {
    let dir = temp_dir();
    let bin = dir.path().join("bin");
    let input = dir.path().join("retrieved.json");
    let output = dir.path().join("reranked.json");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::create_dir(&bin).unwrap();
    write_file(
        &input,
        r#"
{
  "query": "rust graph",
  "strategy": "lexical",
  "matches": [
    {"path": "docs/python.txt", "score": 10, "text": "Python notebooks"},
    {"path": "docs/rust.txt", "score": 1, "text": "Rust graph pipelines"}
  ]
}
"#,
    );
    let command = bin.join("rerank");
    write_file(
        &command,
        r#"#!/bin/sh
request=$(cat)
case "$request" in
  *'"query":"rust graph"'*'"top_k":1'*)
    printf '{"query":"rust graph","strategy":"command","matches":[{"path":"docs/rust.txt","score":0.98,"text":"Rust graph pipelines"}]}'
    ;;
  *)
    printf '%s\n' "$request" >&2
    exit 9
    ;;
esac
"#,
    );
    make_executable(&command);
    write_file(
        &manifest,
        format!(
            r#"
version: 1
inputs:
  retrieved:
    path: {}
    format: json
graph:
  - id: load_retrieved
    op: load
    input: retrieved
  - id: rerank_context
    op: rerank
    from: load_retrieved
    strategy: command
    command: [{}]
    top_k: 1
outputs:
  final:
    from: rerank_context
    path: {}
"#,
            input.display(),
            command.display(),
            output.display()
        ),
    );

    let mut cmd = llmff_cmd();
    cmd.current_dir(dir.path())
        .args(["run", manifest.to_str().unwrap()])
        .assert()
        .success();

    let json: serde_json::Value =
        serde_json::from_str(&read_file(output)).expect("command rerank output should be JSON");
    assert_eq!(json["strategy"], "command");
    assert_eq!(json["matches"].as_array().unwrap().len(), 1);
    assert_eq!(json["matches"][0]["path"], "docs/rust.txt");
    assert_eq!(json["matches"][0]["score"], 0.98);
}

#[test]
fn inline_graph_run_executes_cache_stage() {
    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("answer.txt");
    std::fs::write(&prompt, "first").unwrap();

    let graph = "load | cache(path=.llmff/cache,key=answer-v1) | write(answer.txt)";
    let mut first = llmff_cmd();
    first
        .current_dir(dir.path())
        .args(["run", "-i", prompt.to_str().unwrap(), "-g", graph])
        .assert()
        .success();
    assert_eq!(read_file(&output), "first");

    std::fs::write(&prompt, "second").unwrap();
    let mut second = llmff_cmd();
    second
        .current_dir(dir.path())
        .args(["run", "-i", prompt.to_str().unwrap(), "-g", graph])
        .assert()
        .success();
    assert_eq!(read_file(&output), "first");
}

#[test]
fn inline_graph_run_executes_command_tool_stage() {
    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("tool-output.txt");
    std::fs::write(&prompt, "tool stdin").unwrap();

    let mut cmd = llmff_cmd();
    cmd.current_dir(dir.path())
        .args([
            "run",
            "-i",
            prompt.to_str().unwrap(),
            "-g",
            "load | tool(command=/bin/cat) | write(tool-output.txt)",
        ])
        .assert()
        .success();

    assert_eq!(read_file(output), "tool stdin");
}

#[tokio::test]
async fn inline_graph_run_executes_http_tool_stage() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/process"))
        .respond_with(ResponseTemplate::new(200).set_body_string("tool response"))
        .mount(&server)
        .await;

    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("tool-output.txt");
    std::fs::write(&prompt, "tool body").unwrap();

    let mut cmd = llmff_cmd();
    cmd.current_dir(dir.path())
        .args([
            "run",
            "-i",
            prompt.to_str().unwrap(),
            "-g",
            &format!(
                "load | tool(method=POST,url={}/process) | write(tool-output.txt)",
                server.uri()
            ),
        ])
        .assert()
        .success();

    assert_eq!(read_file(output), "tool response");
}

#[tokio::test]
async fn run_dir_http_server_error_writes_http_failure_contract() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/process"))
        .respond_with(ResponseTemplate::new(500).set_body_string("server failed"))
        .mount(&server)
        .await;

    let dir = temp_dir();
    let prompt = dir.path().join("question.txt");
    let manifest = dir.path().join("pipeline.yaml");
    let run_dir = dir.path().join("run");
    std::fs::write(&prompt, "tool body").unwrap();
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
  - id: call_http
    op: tool
    from: load_prompt
    method: POST
    url: "{}/process"
outputs:
  final:
    from: call_http
    path: answer.txt
"#,
            prompt.display(),
            server.uri()
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
        .stderr(predicate::str::contains("http tool returned status 500"));

    let result = read_run_result(&run_dir);
    assert_eq!(result["exit_code"], 21);
    assert_eq!(result["failure"]["kind"], "http");
    assert_eq!(
        result["failure"]["retry_recommendation"],
        "check_stage_or_input"
    );
    let events = read_text_artifact(&run_dir, "events.jsonl");
    assert!(events.contains(r#""failure_kind":"http""#));
}

#[test]
fn run_executes_plugin_tool_transport() {
    let dir = temp_dir();
    let plugin_dir = dir.path().join("plugins");
    let plugin = plugin_dir.join("cat-plugin");
    std::fs::create_dir_all(&plugin).unwrap();
    write_file(
        plugin.join("llmff-plugin.yaml"),
        r#"
name: cat-plugin
version: 0.1.0
capabilities:
  - kind: tool-transport
    name: stdio-cat
    entrypoint: /bin/cat
"#,
    );
    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("tool-output.txt");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&prompt, "plugin stdin").unwrap();
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
  - id: call_tool
    op: tool
    from: load_prompt
    transport: stdio-cat
outputs:
  final:
    from: call_tool
    path: {}
"#,
            prompt.display(),
            output.display()
        ),
    );

    let mut cmd = llmff_cmd();
    cmd.current_dir(dir.path())
        .args([
            "run",
            manifest.to_str().unwrap(),
            "--plugin-dir",
            plugin_dir.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert_eq!(read_file(output), "plugin stdin");
}

#[test]
fn run_executes_plugin_stage() {
    let dir = temp_dir();
    let plugin_dir = dir.path().join("plugins");
    let plugin = plugin_dir.join("text-plugin");
    let bin = plugin.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    write_file(
        plugin.join("llmff-plugin.yaml"),
        r#"
name: text-plugin
version: 0.1.0
capabilities:
  - kind: stage
    name: text.uppercase
    entrypoint: ./bin/uppercase
"#,
    );
    let entrypoint = bin.join("uppercase");
    std::fs::write(&entrypoint, "#!/bin/sh\ntr '[:lower:]' '[:upper:]'\n").unwrap();
    make_executable(&entrypoint);

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
    cmd.current_dir(dir.path())
        .args([
            "run",
            manifest.to_str().unwrap(),
            "--plugin-dir",
            plugin_dir.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert_eq!(read_file(output), "PLUGIN STAGE");
}

#[test]
fn run_executes_plugin_backend() {
    let dir = temp_dir();
    let plugin_dir = dir.path().join("plugins");
    let plugin = plugin_dir.join("model-plugin");
    let bin = plugin.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    write_file(
        plugin.join("llmff-plugin.yaml"),
        r#"
name: model-plugin
version: 0.1.0
capabilities:
  - kind: backend
    name: local-echo
    entrypoint: ./bin/backend
"#,
    );
    let entrypoint = bin.join("backend");
    write_file(
        &entrypoint,
        "#!/bin/sh\ncat >/dev/null\nprintf '{\"text\":\"plugin backend response\"}'\n",
    );
    make_executable(&entrypoint);

    let prompt = dir.path().join("question.txt");
    let output = dir.path().join("answer.txt");
    let manifest = dir.path().join("pipeline.yaml");
    std::fs::write(&prompt, "ask plugin backend").unwrap();
    write_file(
        &manifest,
        infer_manifest(prompt.display(), "local-echo:test-model", output.display()),
    );

    let mut cmd = llmff_cmd();
    cmd.current_dir(dir.path())
        .args([
            "run",
            manifest.to_str().unwrap(),
            "--plugin-dir",
            plugin_dir.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert_eq!(read_file(output), "plugin backend response");
}

#[test]
fn run_applies_plugin_sampler_before_plugin_backend() {
    let dir = temp_dir();
    let plugin_dir = dir.path().join("plugins");
    let plugin = plugin_dir.join("sampling-plugin");
    let bin = plugin.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    write_file(
        plugin.join("llmff-plugin.yaml"),
        r#"
name: sampling-plugin
version: 0.1.0
capabilities:
  - kind: sampler
    name: safe-small
    entrypoint: ./bin/sampler
  - kind: backend
    name: local-check
    entrypoint: ./bin/backend
"#,
    );
    let sampler = bin.join("sampler");
    write_file(&sampler, "#!/bin/sh\ncat >/dev/null\nprintf '{\"temperature\":0.1,\"max_tokens\":5,\"stop\":[\"DONE\"]}'\n");
    let backend = bin.join("backend");
    write_file(
        &backend,
        r#"#!/bin/sh
request=$(cat)
case "$request" in
  *'"temperature":0.1'*'"max_tokens":5'*'"stop":["DONE"]'*)
    printf '{"text":"sampler applied"}'
    ;;
  *)
    printf '%s\n' "$request" >&2
    exit 9
    ;;
esac
"#,
    );
    make_executable(&sampler);
    make_executable(&backend);

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
    model: local-check:test-model
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
    cmd.current_dir(dir.path())
        .args([
            "run",
            manifest.to_str().unwrap(),
            "--plugin-dir",
            plugin_dir.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert_eq!(read_file(output), "sampler applied");
}

#[test]
fn inline_graph_run_rejects_manifest_and_graph_together() {
    let dir = temp_dir();
    let manifest = dir.path().join("pipeline.yaml");
    write_file(
        &manifest,
        r#"
version: 1
graph: []
"#,
    );

    let mut cmd = llmff_cmd();
    cmd.args(["run", manifest.to_str().unwrap(), "-g", "load | write(-)"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "provide either manifest or --graph",
        ));
}
