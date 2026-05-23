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
fn schema_contract_checker_passes() {
    let root = workspace_root();

    Command::new("python3")
        .arg(root.join("scripts/check-schema-contract.py"))
        .current_dir(root)
        .assert()
        .success();
}
