#![allow(dead_code)]

use assert_cmd::Command;
use std::path::{Path, PathBuf};

pub fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("CLI crate should live under crates/llmff-cli")
        .to_path_buf()
}

/// A `Command` for the built `llmff` binary.
pub fn llmff_cmd() -> Command {
    Command::cargo_bin("llmff").unwrap()
}

pub fn temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

pub fn read_file(path: impl AsRef<Path>) -> String {
    std::fs::read_to_string(path).unwrap()
}

pub fn write_file(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) {
    std::fs::write(path, contents).unwrap()
}

/// Marks `path` executable on unix; a no-op elsewhere (matches the existing permissions).
pub fn make_executable(path: impl AsRef<Path>) {
    let path = path.as_ref();
    let mut permissions = std::fs::metadata(path).unwrap().permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(0o755);
    }
    std::fs::set_permissions(path, permissions).unwrap();
}

pub fn read_text_artifact(dir: &Path, name: &str) -> String {
    read_file(dir.join(name))
}

pub fn read_json_artifact(dir: &Path, name: &str) -> serde_json::Value {
    serde_json::from_str(&read_text_artifact(dir, name)).unwrap()
}

pub fn read_run_result(run_dir: &Path) -> serde_json::Value {
    read_json_artifact(run_dir, "result.json")
}

/// Parse a JSONL file's lines into individual JSON values.
pub fn parse_jsonl(text: &str) -> Vec<serde_json::Value> {
    text.lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

/// A single-stage `load -> infer -> write` manifest, the most common pipeline shape.
pub fn infer_manifest(
    prompt: impl std::fmt::Display,
    model: &str,
    output: impl std::fmt::Display,
) -> String {
    format!(
        r#"
version: 1
inputs:
  prompt:
    path: {prompt}
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: load_prompt
    model: {model}
outputs:
  final:
    from: draft
    path: {output}
"#
    )
}

/// A `load -> write` manifest with no inference stage.
pub fn load_only_manifest(
    prompt: impl std::fmt::Display,
    output: impl std::fmt::Display,
) -> String {
    format!(
        r#"
version: 1
inputs:
  prompt:
    path: {prompt}
graph:
  - id: load_prompt
    op: load
    input: prompt
outputs:
  final:
    from: load_prompt
    path: {output}
"#
    )
}

pub fn missing_backend_plugin_manifest() -> &'static str {
    r#"
name: broken-plugin
version: 0.1.0
capabilities:
  - kind: backend
    name: missing-backend
    entrypoint: ./bin/missing-backend
"#
}

pub fn non_executable_stage_plugin_manifest() -> &'static str {
    r#"
name: broken-plugin
version: 0.1.0
capabilities:
  - kind: stage
    name: text.clean
    entrypoint: ./bin/stage
"#
}

pub fn local_echo_model_plugin_manifest() -> &'static str {
    r#"
name: model-plugin
version: 0.1.0
capabilities:
  - kind: backend
    name: local-echo
    entrypoint: /bin/false
"#
}
