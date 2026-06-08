use assert_cmd::Command;
use std::path::PathBuf;
use std::{fs, path::Path};

const PIPELINE_TEMPLATES: &[(&str, &str)] = &[
    ("Summarization", "examples/templates/summarization.yaml"),
    (
        "Extraction",
        "examples/templates/structured-extraction.yaml",
    ),
    ("Classification", "examples/templates/classification.yaml"),
    ("JSON Repair", "examples/templates/json-repair.yaml"),
    (
        "Self-Refine Loop",
        "examples/templates/self-refine-loop.yaml",
    ),
    ("RAG Answer", "examples/templates/rag-answer.yaml"),
    (
        "Batch Processing",
        "examples/templates/batch-processing.yaml",
    ),
    ("Tool Calling", "examples/templates/tool-calling.yaml"),
    ("Eval Harness", "examples/templates/eval-harness.yaml"),
    (
        "Multi-Provider Fallback",
        "examples/templates/multi-provider-fallback.yaml",
    ),
    (
        "Cost/Latency Comparison",
        "examples/templates/cost-latency-comparison.yaml",
    ),
];

const REAL_WORLD_EXAMPLES: &[(&str, &str)] = &[
    ("Issue Triage", "examples/real-world/issue-triage.yaml"),
    ("Meeting Notes", "examples/real-world/meeting-notes.yaml"),
    ("Local RAG Answer", "examples/real-world/rag-answer.yaml"),
    (
        "Batch Classification",
        "examples/real-world/batch-classification.yaml",
    ),
];

const LOOP_EXAMPLES: &[(&str, &str, &str)] = &[
    (
        "Self-Refining Answer",
        "examples/loops/self-refining-answer-loop.yaml",
        r#"{"answer":"Use llmff for bounded, inspectable LLM pipelines.","confidence":0.93}"#,
    ),
    (
        "ReAct-Style Tool Loop",
        "examples/loops/react-style-tool-use-loop.yaml",
        r#"{"tool":"direct","args":{},"done":true,"final_answer":"Use a bounded loop and inspect the trace."}"#,
    ),
    (
        "Best-of-N Sampling And Selection",
        "examples/loops/best-of-n-sampling+selection-loop.yaml",
        r#"{"candidate":"Candidate answer from a bounded sample.","score":8}"#,
    ),
    (
        "Iterative Research And Fact Check",
        "examples/loops/iterative-research-fact-check-loop.yaml",
        r#"{"supported":true,"claims":["Rust and Python are available in the local context."],"sources":["retrieval/rust.txt","retrieval/python.txt"]}"#,
    ),
    (
        "Map Batch Items",
        "examples/loops/map-batch-items.yaml",
        r#"{}"#,
    ),
];

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("CLI crate should live under crates/llmff-cli")
        .to_path_buf()
}

fn write_fake_eventloom_bin(dir: &Path) -> PathBuf {
    let bin = dir.join("fake-eventloom");
    fs::write(
        &bin,
        r#"#!/usr/bin/env python3
import json
import os
import sys

capture = os.environ["EVENTLOOM_CAPTURE"]
record = {"argv": sys.argv[1:]}
with open(capture, "a", encoding="utf-8") as file:
    file.write(json.dumps(record, separators=(",", ":")) + "\n")
print(json.dumps({"id": "evt_fake", "hash": "sha256:" + "0" * 64, "previousHash": None}))
"#,
    )
    .expect("fake Eventloom script should be writable");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&bin).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&bin, permissions).unwrap();
    }
    bin
}

#[test]
fn product_spec_defines_scope_goal_and_open_items() {
    let root = workspace_root();
    let spec = root.join("SPEC.md");
    assert!(spec.exists(), "missing root SPEC.md");

    let source = std::fs::read_to_string(&spec).expect("SPEC.md should be readable");
    for required in [
        "# llmff Specification",
        "Current Implementation",
        "Product Goal",
        "Supported Execution Contract",
        "Production-Readiness Criteria",
        "Explicitly Not Ready",
        "Functionality Roadmap",
        "Example Roadmap",
        "Distribution And Trust Roadmap",
        "Outside llmff",
        "agent framework",
        "bounded execution tool",
    ] {
        assert!(source.contains(required), "SPEC.md should cover {required}");
    }
}

#[test]
fn adoption_docs_cover_decision_cookbook_and_migration_paths() {
    let root = workspace_root();
    let readme = std::fs::read_to_string(root.join("README.md")).expect("README readable");
    let quickstart =
        std::fs::read_to_string(root.join("docs/quickstart.md")).expect("quickstart readable");
    let decision = std::fs::read_to_string(root.join("docs/when-to-use-llmff.md"))
        .expect("decision guide readable");
    let cookbook =
        std::fs::read_to_string(root.join("docs/cookbook.md")).expect("cookbook readable");
    let migration = std::fs::read_to_string(root.join("docs/migration/pre-1.0-to-1.0.md"))
        .expect("migration guide readable");
    let workflows = std::fs::read_to_string(root.join("docs/agent-workflows.md"))
        .expect("agent workflows readable");

    for path in [
        "docs/when-to-use-llmff.md",
        "docs/cookbook.md",
        "docs/migration/pre-1.0-to-1.0.md",
    ] {
        assert!(readme.contains(path), "README should link {path}");
    }

    for required in [
        "typed inference sub-pipelines",
        "agent framework",
        "model server",
        "scheduler",
        "memory system",
        "autonomous planner",
        "llmff inspect pipeline.yaml --format json",
        "llmff run pipeline.yaml --run-dir",
    ] {
        assert!(
            decision.contains(required),
            "decision guide should cover {required}"
        );
        assert!(
            quickstart.contains("docs/when-to-use-llmff.md"),
            "quickstart should route first-reader decision guidance"
        );
    }

    for required in [
        "offline-runnable by default",
        "examples/templates/rag-answer.yaml",
        "examples/templates/tool-calling.yaml",
        "examples/templates/eval-harness.yaml",
        "examples/templates/batch-processing.yaml",
        "examples/real-world/issue-triage.yaml",
        "examples/agent-workflows/supervisor.py",
        "examples/agent-workflows/batch-supervisor.py",
        "examples/agent-workflows/node-supervisor.mjs",
        "docs/pipeline-library.md",
    ] {
        assert!(
            cookbook.contains(required),
            "cookbook should route to {required}"
        );
    }

    for required in [
        "pre-1.0",
        "llmff inspect <manifest> --format json",
        "--run-dir <dir>",
        "failure_kind",
        "llmff doctor",
        "llmff plugins validate --plugin-dir <dir>",
        "llmff backends report",
    ] {
        assert!(
            migration.contains(required),
            "migration guide should cover {required}"
        );
    }

    for required in [
        "inspect",
        "preserve the original process exit code",
        "run-directory artifacts",
        "safe failure kinds",
        "result.json",
    ] {
        assert!(
            workflows.contains(required),
            "agent workflows should state canonical supervisor pattern: {required}"
        );
    }
}

#[test]
fn pipeline_library_catalog_is_documented_and_inspectable_offline() {
    let root = workspace_root();
    let examples_docs = root.join("examples/README.md");
    let library_docs = root.join("docs/pipeline-library.md");

    assert!(examples_docs.exists(), "missing examples README");
    assert!(library_docs.exists(), "missing pipeline library docs");

    let examples_source =
        std::fs::read_to_string(&examples_docs).expect("README should be readable");
    let library_source =
        std::fs::read_to_string(&library_docs).expect("pipeline docs should be readable");

    for (name, manifest) in PIPELINE_TEMPLATES {
        assert!(
            root.join(manifest).exists(),
            "missing pipeline template {manifest}"
        );
        assert!(
            examples_source.contains(manifest),
            "examples README should list {manifest}"
        );
        assert!(
            library_source.contains(&format!("## {name}")),
            "pipeline docs should document {name}"
        );
        assert!(
            library_source.contains(&format!("llmff inspect {manifest}")),
            "pipeline docs should include inspect command for {manifest}"
        );
        assert!(
            library_source.contains(&format!("llmff run {manifest}")),
            "pipeline docs should include copy-run command for {manifest}"
        );

        Command::cargo_bin("llmff")
            .unwrap()
            .args(["inspect", root.join(manifest).to_str().unwrap()])
            .assert()
            .success();
    }

    for required_note in [
        "Multi-provider fallback is simulated",
        "Cost/latency comparison is simulated",
    ] {
        assert!(
            library_source.contains(required_note),
            "pipeline docs should explain: {required_note}"
        );
    }
}

#[test]
fn real_world_examples_are_documented_and_inspectable_offline() {
    let root = workspace_root();
    let examples_docs = root.join("examples/README.md");
    let examples_source =
        std::fs::read_to_string(&examples_docs).expect("README should be readable");

    for (name, manifest) in REAL_WORLD_EXAMPLES {
        assert!(
            root.join(manifest).exists(),
            "missing real-world example {manifest}"
        );
        assert!(
            examples_source.contains(&format!("### {name}")),
            "examples README should document {name}"
        );
        assert!(
            examples_source.contains(&format!("llmff inspect {manifest}")),
            "examples README should include inspect command for {manifest}"
        );
        assert!(
            examples_source.contains(&format!("llmff run {manifest}")),
            "examples README should include run command for {manifest}"
        );

        Command::cargo_bin("llmff")
            .unwrap()
            .args(["inspect", root.join(manifest).to_str().unwrap()])
            .assert()
            .success();
    }
}

#[test]
fn wisepick_eventloom_flow_example_runs_offline_dry_run() {
    let root = workspace_root();
    let example_dir = root.join("examples/wisepick-eventloom-flow");
    let readme = example_dir.join("README.md");
    let harness = example_dir.join("run.py");
    let out_dir = tempfile::tempdir().expect("temp dir should be created");

    assert!(readme.exists(), "missing WisePick/Eventloom flow README");
    assert!(harness.exists(), "missing WisePick/Eventloom flow harness");

    let readme_source = std::fs::read_to_string(&readme).expect("README should be readable");
    for required in [
        "external composition harness",
        "POST /v1/decide",
        "llmff run",
        "Eventloom-compatible JSONL",
        "POST /v1/feedback",
    ] {
        assert!(
            readme_source.contains(required),
            "README should document boundary phrase: {required}"
        );
    }

    Command::new("python3")
        .args(["-m", "py_compile", harness.to_str().unwrap()])
        .assert()
        .success();

    Command::new("python3")
        .arg(&harness)
        .args([
            "--dry-run",
            "--intent",
            "Clean and return this record as JSON",
            "--out-dir",
            out_dir.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let journal_path = out_dir.path().join("eventloom-compatible.jsonl");
    assert!(journal_path.exists(), "dry-run should write journal");

    let journal = std::fs::read_to_string(journal_path).expect("journal should be readable");
    for event_type in [
        "\"type\":\"routing.decide.requested\"",
        "\"type\":\"routing.decided\"",
        "\"type\":\"llmff.execution.planned\"",
        "\"type\":\"routing.feedback.planned\"",
    ] {
        assert!(
            journal.contains(event_type),
            "journal should contain event type {event_type}"
        );
    }

    let run_dir = tempfile::tempdir().expect("run temp dir should be created");
    let llmff_bin = Command::cargo_bin("llmff")
        .unwrap()
        .get_program()
        .to_owned();
    Command::new("python3")
        .arg(&harness)
        .args([
            "--mock-wisepick",
            "--intent",
            "Clean and return this record as JSON",
            "--out-dir",
            run_dir.path().to_str().unwrap(),
            "--llmff-bin",
            llmff_bin.to_str().unwrap(),
        ])
        .assert()
        .success();

    let run_journal_path = run_dir.path().join("eventloom-compatible.jsonl");
    let run_journal = std::fs::read_to_string(run_journal_path).expect("journal readable");
    for event_type in [
        "\"type\":\"llmff.execution.started\"",
        "\"type\":\"llmff.execution.finished\"",
        "\"type\":\"routing.feedback.planned\"",
    ] {
        assert!(
            run_journal.contains(event_type),
            "mock-WisePick journal should contain event type {event_type}"
        );
    }

    let import_dir = tempfile::tempdir().expect("import temp dir should be created");
    let fake_eventloom = write_fake_eventloom_bin(import_dir.path());
    let capture_path = import_dir.path().join("eventloom-append-calls.jsonl");
    let eventloom_log = import_dir.path().join("sealed-eventloom.jsonl");
    Command::new("python3")
        .arg(&harness)
        .env("EVENTLOOM_CAPTURE", &capture_path)
        .args([
            "--dry-run",
            "--intent",
            "Clean and return this record as JSON",
            "--out-dir",
            import_dir.path().to_str().unwrap(),
            "--eventloom-log",
            eventloom_log.to_str().unwrap(),
            "--eventloom-bin",
            fake_eventloom.to_str().unwrap(),
        ])
        .assert()
        .success();

    let append_calls =
        std::fs::read_to_string(&capture_path).expect("fake Eventloom should capture append calls");
    for expected in [
        "\"append\"",
        "\"routing.decide.requested\"",
        "\"routing.decided\"",
        "\"llmff.execution.planned\"",
        "\"routing.feedback.planned\"",
        eventloom_log.to_str().unwrap(),
    ] {
        assert!(
            append_calls.contains(expected),
            "Eventloom append calls should contain {expected}"
        );
    }
}

#[test]
fn loop_examples_are_documented_inspectable_and_runnable_offline() {
    let root = workspace_root();
    let readme_path = root.join("examples/loops/README.md");
    let readme = std::fs::read_to_string(&readme_path).expect("loop README should be readable");
    let examples_source = std::fs::read_to_string(root.join("examples/README.md"))
        .expect("README should be readable");
    let quickstart = std::fs::read_to_string(root.join("docs/quickstart.md"))
        .expect("quickstart should be readable");
    let pipeline_library = std::fs::read_to_string(root.join("docs/pipeline-library.md"))
        .expect("pipeline library should be readable");

    assert!(
        quickstart.contains("Run A Bounded Loop"),
        "quickstart should introduce the v1.1 loop path"
    );
    assert!(
        examples_source.contains("examples/loops/README.md"),
        "examples README should link the loop catalog"
    );
    assert!(
        pipeline_library.contains("Loop Example Catalog"),
        "pipeline library should route loop adoption examples"
    );

    for (name, manifest, mock_response) in LOOP_EXAMPLES {
        let manifest_path = root.join(manifest);
        assert!(manifest_path.exists(), "missing loop example {manifest}");
        assert!(
            readme.contains(&format!("## {name}")),
            "loop README should document {name}"
        );
        assert!(
            readme.contains(&format!("llmff inspect {manifest}")),
            "loop README should include inspect command for {manifest}"
        );
        assert!(
            readme.contains(&format!("llmff run {manifest}")),
            "loop README should include run command for {manifest}"
        );

        Command::cargo_bin("llmff")
            .unwrap()
            .args(["inspect", manifest_path.to_str().unwrap()])
            .assert()
            .success();

        Command::cargo_bin("llmff")
            .unwrap()
            .env("LLMFF_MOCK_GOOD_RESPONSE", mock_response)
            .args(["run", manifest_path.to_str().unwrap()])
            .assert()
            .success();
    }

    for path in [
        "self-refining-answer.output.json",
        "react-style-tool-use.output.json",
        "best-of-n-sampling.output.json",
        "iterative-research-fact-check.output.json",
        "map-batch-items.output.json",
    ] {
        let _ = std::fs::remove_file(root.join("examples/loops").join(path));
    }
}

#[test]
fn real_world_examples_run_with_mock_backends() {
    let root = workspace_root();
    let output_dir = root.join("examples/real-world/outputs");

    let issue_output = output_dir.join("issue-triage.json");
    let meeting_output = output_dir.join("meeting-notes.json");
    let rag_output = output_dir.join("rag-answer.txt");
    for output in [&issue_output, &meeting_output, &rag_output] {
        let _ = std::fs::remove_file(output);
    }

    Command::cargo_bin("llmff")
        .unwrap()
        .env(
            "LLMFF_MOCK_GOOD_RESPONSE",
            r#"{"category":"operations","priority":"high","summary":"Nightly invoice export times out before finance close.","recommended_action":"Escalate to the job owner and provide a same-day workaround."}"#,
        )
        .args([
            "run",
            root.join("examples/real-world/issue-triage.yaml")
                .to_str()
                .unwrap(),
        ])
        .assert()
        .success();
    assert!(issue_output.exists(), "issue triage should write output");

    Command::cargo_bin("llmff")
        .unwrap()
        .env(
            "LLMFF_MOCK_GOOD_RESPONSE",
            r#"{"summary":"The team kept llmff focused on bounded execution.","decisions":["llmff remains an execution substrate."],"actions":[{"owner":"Dana","task":"Draft production examples."}]}"#,
        )
        .args([
            "run",
            root.join("examples/real-world/meeting-notes.yaml")
                .to_str()
                .unwrap(),
        ])
        .assert()
        .success();
    assert!(meeting_output.exists(), "meeting notes should write output");

    Command::cargo_bin("llmff")
        .unwrap()
        .env(
            "LLMFF_MOCK_GOOD_RESPONSE",
            "Use llmff as a bounded subprocess with explicit artifacts.",
        )
        .args([
            "run",
            root.join("examples/real-world/rag-answer.yaml")
                .to_str()
                .unwrap(),
        ])
        .assert()
        .success();
    assert!(rag_output.exists(), "RAG answer should write output");

    let batch_output = root.join("examples/real-world/outputs/batch-items");
    let _ = std::fs::remove_dir_all(batch_output.join("items"));
    let _ = std::fs::remove_dir_all(batch_output.join("inputs"));
    let _ = std::fs::remove_file(batch_output.join("batch-report.jsonl"));
    Command::cargo_bin("llmff")
        .unwrap()
        .env(
            "LLMFF_MOCK_GOOD_RESPONSE",
            r#"{"label":"support","confidence":0.91,"rationale":"The item asks for operational guidance."}"#,
        )
        .args([
            "run",
            "--batch-input",
            root.join("examples/real-world/inputs/batch-items.jsonl")
                .to_str()
                .unwrap(),
            "--batch-output-dir",
            batch_output.to_str().unwrap(),
            root.join("examples/real-world/batch-classification.yaml")
                .to_str()
                .unwrap(),
        ])
        .assert()
        .success();
    assert!(
        batch_output.join("batch-report.jsonl").exists(),
        "batch classification should write a batch report"
    );

    for output in [issue_output, meeting_output, rag_output] {
        let _ = std::fs::remove_file(output);
    }
    let _ = std::fs::remove_dir_all(batch_output.join("items"));
    let _ = std::fs::remove_dir_all(batch_output.join("inputs"));
    let _ = std::fs::remove_file(batch_output.join("batch-report.jsonl"));
}

#[test]
fn real_world_issue_triage_links_to_a_runnable_supervisor_example() {
    let root = workspace_root();
    let examples_readme =
        std::fs::read_to_string(root.join("examples/README.md")).expect("examples README readable");
    let supervisor = root.join("examples/real-world/supervisor.py");

    assert!(supervisor.exists(), "missing real-world supervisor example");
    assert!(examples_readme.contains("examples/real-world/supervisor.py"));

    let temp = tempfile::tempdir().expect("tempdir should be available");
    Command::new("python3")
        .arg(supervisor)
        .arg("--run-dir")
        .arg(temp.path())
        .current_dir(&root)
        .env("LLMFF_BIN", assert_cmd::cargo::cargo_bin("llmff"))
        .assert()
        .success()
        .stdout(predicates::str::contains("inspect="))
        .stdout(predicates::str::contains("trace="))
        .stdout(predicates::str::contains("events="))
        .stdout(predicates::str::contains("run_status=ok"))
        .stdout(predicates::str::contains(
            "output=examples/real-world/outputs/issue-triage.json",
        ))
        .stdout(predicates::str::contains("output_exists=true"));

    assert!(
        temp.path().join("inspect.json").exists(),
        "supervisor should save inspect.json"
    );
    assert!(
        temp.path().join("trace.jsonl").exists(),
        "supervisor should save trace.jsonl"
    );
    assert!(
        temp.path().join("events.jsonl").exists(),
        "supervisor should save events.jsonl"
    );

    let _ = std::fs::remove_file(root.join("examples/real-world/outputs/issue-triage.json"));
}

#[test]
fn provider_onboarding_examples_are_inspectable() {
    let root = workspace_root();
    let docs = root.join("docs/provider-troubleshooting.md");
    assert!(docs.exists(), "missing provider troubleshooting docs");

    let docs_source = std::fs::read_to_string(&docs).expect("docs should be readable");
    for required in [
        "API key lookup",
        "base URL normalization",
        "JSON response-format support",
        "token streaming support",
        "common HTTP failure modes",
    ] {
        assert!(
            docs_source.contains(required),
            "provider docs should cover {required}"
        );
    }

    for manifest in [
        "examples/providers/openai-compatible.mock.yaml",
        "examples/providers/ollama.mock.yaml",
    ] {
        Command::cargo_bin("llmff")
            .unwrap()
            .args(["inspect", root.join(manifest).to_str().unwrap()])
            .assert()
            .success();
    }

    Command::cargo_bin("llmff")
        .unwrap()
        .args([
            "inspect",
            root.join("examples/providers/openai-compatible.yaml")
                .to_str()
                .unwrap(),
            "--backend",
            "openai=https://api.openai.com",
            "--api-key",
            "openai=test-key",
        ])
        .assert()
        .success();

    Command::cargo_bin("llmff")
        .unwrap()
        .args([
            "inspect",
            root.join("examples/providers/ollama.yaml")
                .to_str()
                .unwrap(),
            "--ollama",
            "ollama=http://localhost:11434/",
        ])
        .assert()
        .success();
}

#[test]
fn provider_live_smoke_scripts_are_explicitly_opt_in() {
    let root = workspace_root();

    for script in [
        "scripts/smoke-openai-compatible-provider.sh",
        "scripts/smoke-ollama-provider.sh",
    ] {
        let path = root.join(script);
        assert!(path.exists(), "missing {script}");

        let source = std::fs::read_to_string(&path).expect("script should be readable");
        assert!(
            source.contains("LLMFF_LIVE_PROVIDER_SMOKE=1"),
            "{script} should require LLMFF_LIVE_PROVIDER_SMOKE=1"
        );
        assert!(
            source.contains("exit 0"),
            "{script} should skip cleanly without opt-in"
        );
    }

    let openai = Command::new(root.join("scripts/smoke-openai-compatible-provider.sh"))
        .assert()
        .success();
    openai.stdout(predicates::str::contains(
        "skipping OpenAI-compatible provider smoke",
    ));

    let ollama = Command::new(root.join("scripts/smoke-ollama-provider.sh"))
        .assert()
        .success();
    ollama.stdout(predicates::str::contains("skipping Ollama provider smoke"));
}

#[test]
fn provider_live_smoke_readiness_is_checked() {
    let root = workspace_root();
    let guide = root.join("docs/provider-smoke-readiness.md");
    assert!(guide.exists(), "missing provider smoke readiness guide");

    let source = std::fs::read_to_string(&guide).expect("guide should be readable");
    for required in [
        "LLMFF_LIVE_PROVIDER_SMOKE=1",
        "OPENAI_API_KEY",
        "OPENAI_BASE_URL",
        "OLLAMA_BASE_URL",
        "ubuntu-latest",
        "workflow_dispatch",
        "not run on pull_request or push",
        "certification is a support commitment",
    ] {
        assert!(
            source.contains(required),
            "provider smoke readiness guide should cover {required}"
        );
    }

    Command::new(root.join("scripts/check-provider-smoke-readiness.sh"))
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "provider smoke readiness validation succeeded",
        ));
}

#[test]
fn provider_docs_examples_and_ci_cover_common_gateways() {
    let root = workspace_root();
    let provider_dir = root.join("docs/providers");
    assert!(provider_dir.exists(), "missing provider docs directory");

    for provider in [
        "openai",
        "azure-openai",
        "lm-studio",
        "vllm",
        "localai",
        "openrouter",
        "together",
        "groq",
    ] {
        let doc = provider_dir.join(format!("{provider}.md"));
        assert!(doc.exists(), "missing provider doc for {provider}");
        let source = std::fs::read_to_string(&doc).expect("provider doc should be readable");
        assert!(
            source.contains("llmff backends report"),
            "{provider} doc should show compatibility reporting"
        );
        assert!(
            source.contains("JSON mode"),
            "{provider} doc should mention JSON mode compatibility"
        );
    }

    let anthropic = provider_dir.join("anthropic.md");
    assert!(anthropic.exists(), "missing Anthropic provider note");
    let anthropic_source =
        std::fs::read_to_string(&anthropic).expect("Anthropic doc should be readable");
    assert!(anthropic_source.contains("adapter-only"));

    for manifest in [
        "openai.yaml",
        "azure-openai.yaml",
        "lm-studio.yaml",
        "vllm.yaml",
        "localai.yaml",
        "openrouter.yaml",
        "together.yaml",
        "groq.yaml",
    ] {
        Command::cargo_bin("llmff")
            .unwrap()
            .args([
                "inspect",
                root.join("examples/providers")
                    .join(manifest)
                    .to_str()
                    .unwrap(),
                "--backend",
                "provider=https://example.test/v1",
                "--api-key",
                "provider=test-key",
            ])
            .assert()
            .success();
    }

    let workflow = root.join(".github/workflows/live-provider-smoke.yml");
    assert!(workflow.exists(), "missing live provider smoke workflow");
    let workflow_source = std::fs::read_to_string(workflow).expect("workflow should be readable");
    assert!(workflow_source.contains("workflow_dispatch"));
    assert!(!workflow_source.contains("pull_request"));
    assert!(!workflow_source.contains("push:"));
    assert!(workflow_source.contains("LLMFF_LIVE_PROVIDER_SMOKE: \"1\""));
    assert!(workflow_source.contains("secrets.OPENAI_API_KEY"));
}

#[test]
fn agent_workflow_docs_link_to_a_runnable_supervisor_example() {
    let root = workspace_root();
    let docs = root.join("docs/agent-workflows.md");
    let example = root.join("examples/agent-workflows/supervisor.py");
    let batch_example = root.join("examples/agent-workflows/batch-supervisor.py");
    let node_example = root.join("examples/agent-workflows/node-supervisor.mjs");

    assert!(docs.exists(), "missing agent workflow docs");
    assert!(example.exists(), "missing agent supervisor example");
    assert!(
        batch_example.exists(),
        "missing batch agent supervisor example"
    );
    assert!(
        node_example.exists(),
        "missing Node agent supervisor example"
    );

    let readme =
        std::fs::read_to_string(root.join("README.md")).expect("README should be readable");
    let examples_readme =
        std::fs::read_to_string(root.join("examples/README.md")).expect("examples README readable");
    let observability = std::fs::read_to_string(root.join("docs/observability.md"))
        .expect("observability readable");
    let docs_source = std::fs::read_to_string(&docs).expect("agent docs should be readable");

    for required in [
        "subprocess",
        "Short Jobs",
        "Long Jobs",
        "Batch Jobs",
        "Streaming Jobs",
        "--events",
        "--trace",
        "failure_kind",
        "checkpoint",
        "exit code",
    ] {
        assert!(
            docs_source.contains(required),
            "agent docs should cover {required}"
        );
    }

    assert!(readme.contains("docs/agent-workflows.md"));
    assert!(examples_readme.contains("examples/agent-workflows/supervisor.py"));
    assert!(examples_readme.contains("examples/agent-workflows/batch-supervisor.py"));
    assert!(examples_readme.contains("examples/agent-workflows/node-supervisor.mjs"));
    assert!(observability.contains("docs/agent-workflows.md"));
    assert!(docs_source.contains("python3 examples/agent-workflows/batch-supervisor.py"));
    assert!(docs_source.contains("node examples/agent-workflows/node-supervisor.mjs"));

    let temp = tempfile::tempdir().expect("tempdir should be available");
    Command::new("python3")
        .arg(example)
        .arg("--work-dir")
        .arg(temp.path())
        .env("LLMFF_BIN", assert_cmd::cargo::cargo_bin("llmff"))
        .env("LLMFF_MOCK_BAD_RESPONSE", r#"{"wrong":true}"#)
        .env("LLMFF_MOCK_GOOD_RESPONSE", r#"{"answer":"ok"}"#)
        .assert()
        .success()
        .stdout(predicates::str::contains("inspect_format_version=1"))
        .stdout(predicates::str::contains("manifest_hash=sha256:"))
        .stdout(predicates::str::contains("stdout_manifest_outputs=false"))
        .stdout(predicates::str::contains("run_status=ok"))
        .stdout(predicates::str::contains("output_exists=true"));

    let batch_temp = tempfile::tempdir().expect("tempdir should be available");
    Command::new("python3")
        .arg(batch_example)
        .arg("--work-dir")
        .arg(batch_temp.path())
        .current_dir(&root)
        .env("LLMFF_BIN", "target/debug/llmff")
        .assert()
        .success()
        .stdout(predicates::str::contains("inspect_format_version=1"))
        .stdout(predicates::str::contains("manifest_hash=sha256:"))
        .stdout(predicates::str::contains("stdout_manifest_outputs=false"))
        .stdout(predicates::str::contains("run_status=ok"))
        .stdout(predicates::str::contains("batch_report="))
        .stdout(predicates::str::contains("item_count=2"))
        .stdout(predicates::str::contains("failed_count=0"))
        .stdout(predicates::str::contains("item_000000_output_exists=true"))
        .stdout(predicates::str::contains("item_000001_output_exists=true"));

    let node_temp = tempfile::tempdir().expect("tempdir should be available");
    Command::new("node")
        .arg(node_example)
        .arg("--work-dir")
        .arg(node_temp.path())
        .env("LLMFF_BIN", assert_cmd::cargo::cargo_bin("llmff"))
        .env("LLMFF_MOCK_BAD_RESPONSE", r#"{"wrong":true}"#)
        .env("LLMFF_MOCK_GOOD_RESPONSE", r#"{"answer":"ok"}"#)
        .assert()
        .success()
        .stdout(predicates::str::contains("inspect_format_version=1"))
        .stdout(predicates::str::contains("manifest_hash=sha256:"))
        .stdout(predicates::str::contains("stdout_manifest_outputs=false"))
        .stdout(predicates::str::contains("run_status=ok"))
        .stdout(predicates::str::contains("event_count="))
        .stdout(predicates::str::contains("output_exists=true"));
}

#[test]
fn observability_docs_link_to_a_runnable_same_run_example() {
    let root = workspace_root();
    let script = root.join("examples/supervision/local-observability.sh");
    let docs = std::fs::read_to_string(root.join("docs/observability.md"))
        .expect("observability readable");
    let dashboard = std::fs::read_to_string(root.join("examples/supervision/dashboard.md"))
        .expect("dashboard docs readable");

    assert!(script.exists(), "missing local observability example");
    assert!(docs.contains("examples/supervision/local-observability.sh"));
    assert!(dashboard.contains("examples/supervision/local-observability.sh"));

    let temp = tempfile::tempdir().expect("tempdir should be available");
    Command::new("bash")
        .arg(script)
        .arg("--work-dir")
        .arg(temp.path())
        .env("LLMFF_BIN", assert_cmd::cargo::cargo_bin("llmff"))
        .env("LLMFF_MOCK_BAD_RESPONSE", r#"{"wrong":true}"#)
        .env("LLMFF_MOCK_GOOD_RESPONSE", r#"{"answer":"ok"}"#)
        .assert()
        .success()
        .stdout(predicates::str::contains("run_status=ok"))
        .stdout(predicates::str::contains("live_event_count="))
        .stdout(predicates::str::contains("trace="))
        .stdout(predicates::str::contains("events="))
        .stdout(predicates::str::contains("summary="))
        .stdout(predicates::str::contains("metrics="))
        .stdout(predicates::str::contains("summary_has_timing=true"))
        .stdout(predicates::str::contains("metrics_has_run_duration=true"))
        .stdout(predicates::str::contains("output_exists=true"));
}
