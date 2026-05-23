use assert_cmd::Command;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("CLI crate should live under crates/llmff-cli")
        .to_path_buf()
}

#[test]
fn plugin_ecosystem_assets_are_ci_checkable() {
    let root = workspace_root();
    let llmff_bin = assert_cmd::cargo::cargo_bin("llmff");

    Command::new(root.join("scripts/check-plugin-fixtures.sh"))
        .env("LLMFF_BIN", llmff_bin)
        .assert()
        .success()
        .stdout(predicates::str::contains("plugin fixtures ok"));

    let registry_path = root.join("docs/plugins/registry.v1.json");
    let registry: Value = serde_json::from_str(
        &std::fs::read_to_string(&registry_path).expect("registry should be readable"),
    )
    .expect("registry should be JSON");

    assert_eq!(registry["format_version"], 1);
    assert_eq!(registry["plugin_protocol_version"], 1);

    let categories = registry["plugins"]
        .as_array()
        .expect("registry plugins should be an array")
        .iter()
        .map(|plugin| plugin["category"].as_str().unwrap())
        .collect::<BTreeSet<_>>();

    for category in [
        "retrieval-provider",
        "reranker",
        "model-backend",
        "sampler",
        "tool-transport",
        "postprocessor",
    ] {
        assert!(
            categories.contains(category),
            "registry should include {category}"
        );
    }
}
