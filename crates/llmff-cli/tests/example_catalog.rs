use assert_cmd::Command;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("CLI crate should live under crates/llmff-cli")
        .to_path_buf()
}

#[test]
fn provider_onboarding_examples_and_templates_are_inspectable() {
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
        "examples/templates/summarization.yaml",
        "examples/templates/structured-extraction.yaml",
        "examples/templates/multi-step-extraction.yaml",
        "examples/templates/batch-processing.yaml",
        "examples/templates/json-repair.yaml",
        "examples/templates/retrieve-rerank-answer.yaml",
        "examples/templates/tool-call.yaml",
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
