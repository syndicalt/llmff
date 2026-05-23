use assert_cmd::Command;
use std::path::PathBuf;

const PIPELINE_TEMPLATES: &[(&str, &str)] = &[
    ("Summarization", "examples/templates/summarization.yaml"),
    (
        "Extraction",
        "examples/templates/structured-extraction.yaml",
    ),
    ("Classification", "examples/templates/classification.yaml"),
    ("JSON Repair", "examples/templates/json-repair.yaml"),
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

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("CLI crate should live under crates/llmff-cli")
        .to_path_buf()
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
    let node_example = root.join("examples/agent-workflows/node-supervisor.mjs");

    assert!(docs.exists(), "missing agent workflow docs");
    assert!(example.exists(), "missing agent supervisor example");
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
    assert!(examples_readme.contains("examples/agent-workflows/node-supervisor.mjs"));
    assert!(observability.contains("docs/agent-workflows.md"));
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
