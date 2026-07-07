mod common;

use common::*;
use predicates::prelude::*;

#[test]
fn stages_list_prints_builtin_stages() {
    let mut cmd = llmff_cmd();

    cmd.args(["stages", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("cache"))
        .stdout(predicate::str::contains("infer"))
        .stdout(predicate::str::contains("retrieve"))
        .stdout(predicate::str::contains("validate_json"));
}

#[test]
fn stages_list_json_prints_stage_metadata() {
    let mut cmd = llmff_cmd();

    let output = cmd
        .args(["stages", "list", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stages: serde_json::Value =
        serde_json::from_slice(&output).expect("stage list should be valid JSON");

    let infer = stages
        .as_array()
        .expect("stage list should be an array")
        .iter()
        .find(|stage| stage["name"] == "infer")
        .expect("infer stage should be listed");
    assert_eq!(infer["kind"], "model");
    assert!(infer["required_fields"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("model")));
    assert!(infer["capabilities"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("sampling")));

    let tool = stages
        .as_array()
        .unwrap()
        .iter()
        .find(|stage| stage["name"] == "tool")
        .expect("tool stage should be listed");
    assert_eq!(tool["kind"], "integration");
    assert!(tool["required_fields"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("command|url|transport")));
    assert!(tool["capabilities"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("plugin-tool-transport")));
}

#[test]
fn doctor_reports_version_and_writable_run_dir() {
    let dir = temp_dir();
    let run_dir = dir.path().join("run");

    llmff_cmd()
        .args(["doctor", "--run-dir", run_dir.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("version"))
        .stdout(predicate::str::contains("run-dir"))
        .stdout(predicate::str::contains("writable"));
}

#[test]
fn doctor_validates_plugin_dir_without_pipeline_run() {
    let directory = temp_dir();
    let plugin = directory.path().join("broken-plugin");
    std::fs::create_dir(&plugin).unwrap();
    write_file(
        plugin.join("llmff-plugin.yaml"),
        missing_backend_plugin_manifest(),
    );

    llmff_cmd()
        .args(["doctor", "--plugin-dir", directory.path().to_str().unwrap()])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("missing entrypoint"))
        .stderr(predicate::str::contains("missing-backend"));
}

#[test]
fn doctor_checks_api_key_env_without_printing_secret() {
    let secret = "super-secret-doctor-token";
    let env_name = "LLMFF_DOCTOR_TEST_KEY";

    llmff_cmd()
        .args([
            "doctor",
            "--backend",
            "openai=https://api.example.test/v1",
            "--api-key-env",
            &format!("openai={env_name}"),
        ])
        .env(env_name, secret)
        .assert()
        .success()
        .stdout(predicate::str::contains("api-key-env"))
        .stdout(predicate::str::contains(env_name))
        .stdout(predicate::str::contains(secret).not())
        .stderr(predicate::str::contains(secret).not());
}

#[test]
fn backends_list_prints_ollama_backend() {
    let mut cmd = llmff_cmd();

    cmd.args(["backends", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("mock:good"))
        .stdout(predicate::str::contains("ollama"));
}

#[test]
fn backends_list_json_prints_backend_capabilities() {
    let mut cmd = llmff_cmd();

    let output = cmd
        .args(["backends", "list", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let backends: serde_json::Value =
        serde_json::from_slice(&output).expect("backend list should be valid JSON");

    let openai = backends
        .as_array()
        .expect("backend list should be an array")
        .iter()
        .find(|backend| backend["name"] == "openai-compatible")
        .expect("OpenAI-compatible backend should be listed");
    assert_eq!(openai["kind"], "remote-chat");
    assert_eq!(openai["registration_flag"], "--backend <alias>=<base-url>");
    assert_eq!(openai["requires_api_key"], true);
    assert!(openai["capabilities"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("usage-metadata")));
    assert!(openai["capabilities"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("streaming-inference")));
}

#[test]
fn backends_list_json_includes_cli_registered_backend_metadata() {
    let mut cmd = llmff_cmd();

    let output = cmd
        .args([
            "backends",
            "list",
            "--format",
            "json",
            "--backend",
            "openai_alt=https://api.example.test/v1",
            "--ollama",
            "local=http://localhost:11434",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let backends: serde_json::Value =
        serde_json::from_slice(&output).expect("backend list should be valid JSON");

    let openai_alt = backends
        .as_array()
        .expect("backend list should be an array")
        .iter()
        .find(|backend| backend["name"] == "openai_alt")
        .expect("CLI OpenAI-compatible backend should be listed");
    assert_eq!(openai_alt["kind"], "openai-compatible");
    assert_eq!(
        openai_alt["registration_flag"],
        "--backend openai_alt=<base-url>"
    );
    assert_eq!(openai_alt["requires_api_key"], true);
    assert!(openai_alt["model_aliases"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("openai_alt:<model>")));

    let local = backends
        .as_array()
        .unwrap()
        .iter()
        .find(|backend| backend["name"] == "local")
        .expect("CLI Ollama backend should be listed");
    assert_eq!(local["kind"], "ollama");
    assert_eq!(local["registration_flag"], "--ollama local=<base-url>");
    assert_eq!(local["requires_api_key"], false);
}

#[test]
fn backends_report_json_prints_static_provider_compatibility() {
    let mut cmd = llmff_cmd();

    let output = cmd
        .args([
            "backends",
            "report",
            "--format",
            "json",
            "--backend",
            "openrouter=https://openrouter.ai/api/v1",
            "--ollama",
            "local=http://localhost:11434",
            "--api-key-env",
            "openrouter=OPENROUTER_API_KEY",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value =
        serde_json::from_slice(&output).expect("backend report should be valid JSON");

    let openrouter = report
        .as_array()
        .expect("backend report should be an array")
        .iter()
        .find(|backend| backend["name"] == "openrouter")
        .expect("OpenRouter-compatible backend should be reported");
    assert_eq!(openrouter["kind"], "openai-compatible");
    assert_eq!(openrouter["base_url"], "https://openrouter.ai/api/v1");
    assert_eq!(openrouter["api_key_configured"], true);
    assert_eq!(openrouter["capabilities"]["json_mode"]["supported"], true);
    assert_eq!(openrouter["capabilities"]["streaming"]["supported"], true);
    assert_eq!(openrouter["capabilities"]["seed"]["supported"], true);
    assert_eq!(openrouter["capabilities"]["stop"]["supported"], true);
    assert_eq!(
        openrouter["capabilities"]["usage_metadata"]["supported"],
        true
    );
    assert_eq!(openrouter["diagnostics"].as_array().unwrap().len(), 0);

    let local = report
        .as_array()
        .unwrap()
        .iter()
        .find(|backend| backend["name"] == "local")
        .expect("Ollama backend should be reported");
    assert_eq!(local["kind"], "ollama");
    assert_eq!(local["api_key_configured"], false);
    assert_eq!(local["capabilities"]["json_mode"]["supported"], true);
    assert_eq!(local["capabilities"]["streaming"]["supported"], false);
    assert!(local["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|diagnostic| diagnostic["code"] == "streaming_not_supported"));
}

#[test]
fn backends_report_warns_when_openai_compatible_key_is_not_configured() {
    let output = llmff_cmd()
        .args([
            "backends",
            "report",
            "--format",
            "json",
            "--backend",
            "gateway=https://gateway.example.test/v1",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value =
        serde_json::from_slice(&output).expect("backend report should be valid JSON");

    let gateway = report
        .as_array()
        .unwrap()
        .iter()
        .find(|backend| backend["name"] == "gateway")
        .expect("registered gateway should be reported");
    assert_eq!(gateway["api_key_configured"], false);
    assert!(gateway["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|diagnostic| diagnostic["code"] == "api_key_missing"));
}

#[test]
fn backends_list_json_includes_plugin_backend_metadata() {
    let dir = temp_dir();
    let plugin_dir = dir.path().join("plugins");
    let plugin = plugin_dir.join("model-plugin");
    std::fs::create_dir_all(&plugin).unwrap();
    write_file(
        plugin.join("llmff-plugin.yaml"),
        local_echo_model_plugin_manifest(),
    );

    let mut cmd = llmff_cmd();
    let output = cmd
        .args([
            "backends",
            "list",
            "--format",
            "json",
            "--plugin-dir",
            plugin_dir.to_str().unwrap(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let backends: serde_json::Value =
        serde_json::from_slice(&output).expect("backend list should be valid JSON");

    let plugin_backend = backends
        .as_array()
        .expect("backend list should be an array")
        .iter()
        .find(|backend| backend["name"] == "local-echo")
        .expect("plugin backend should be listed");
    assert_eq!(plugin_backend["kind"], "plugin-command");
    assert_eq!(plugin_backend["registration_flag"], "--plugin-dir");
    assert_eq!(plugin_backend["requires_api_key"], false);
    assert!(plugin_backend["capabilities"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("chat-messages")));
    assert!(plugin_backend["model_aliases"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("local-echo:<model>")));
}

#[test]
fn models_list_json_includes_runtime_registered_models() {
    let mut cmd = llmff_cmd();

    let output = cmd
        .args([
            "models",
            "list",
            "--format",
            "json",
            "--backend",
            "openai_alt=https://api.example.test/v1",
            "--ollama",
            "local=http://localhost:11434",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let models: serde_json::Value =
        serde_json::from_slice(&output).expect("model list should be valid JSON");

    let openai_model = models
        .as_array()
        .expect("model list should be an array")
        .iter()
        .find(|model| model["model"] == "openai_alt:<model>")
        .expect("CLI OpenAI-compatible model should be listed");
    assert_eq!(openai_model["backend"], "openai_alt");
    assert_eq!(openai_model["backend_kind"], "openai-compatible");
    assert_eq!(openai_model["runtime"], "remote-chat");
    assert_eq!(openai_model["source"], "cli");
    assert_eq!(openai_model["requires_api_key"], true);
    assert!(openai_model["capabilities"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("streaming-inference")));

    let ollama_model = models
        .as_array()
        .unwrap()
        .iter()
        .find(|model| model["model"] == "local:<model>")
        .expect("CLI Ollama model should be listed");
    assert_eq!(ollama_model["backend"], "local");
    assert_eq!(ollama_model["runtime"], "local-chat");
    assert_eq!(ollama_model["source"], "cli");
    assert_eq!(ollama_model["requires_api_key"], false);
}

#[test]
fn models_list_json_includes_plugin_backend_models() {
    let dir = temp_dir();
    let plugin_dir = dir.path().join("plugins");
    let plugin = plugin_dir.join("model-plugin");
    std::fs::create_dir_all(&plugin).unwrap();
    write_file(
        plugin.join("llmff-plugin.yaml"),
        local_echo_model_plugin_manifest(),
    );

    let mut cmd = llmff_cmd();
    let output = cmd
        .args([
            "models",
            "list",
            "--format",
            "json",
            "--plugin-dir",
            plugin_dir.to_str().unwrap(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let models: serde_json::Value =
        serde_json::from_slice(&output).expect("model list should be valid JSON");

    let plugin_model = models
        .as_array()
        .expect("model list should be an array")
        .iter()
        .find(|model| model["model"] == "local-echo:<model>")
        .expect("plugin backend model should be listed");
    assert_eq!(plugin_model["backend"], "local-echo");
    assert_eq!(plugin_model["backend_kind"], "plugin-command");
    assert_eq!(plugin_model["runtime"], "command");
    assert_eq!(plugin_model["source"], "plugin");
    assert!(plugin_model["capabilities"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("usage-metadata")));
}

#[test]
fn plugins_list_json_prints_discovered_plugin_manifests() {
    let directory = temp_dir();
    std::fs::create_dir(directory.path().join("json-tools")).unwrap();
    write_file(
        directory
            .path()
            .join("json-tools")
            .join("llmff-plugin.yaml"),
        r#"
name: json-tools
version: 0.1.0
capabilities:
  - kind: stage
    name: json.flatten
    entrypoint: ./json_flatten
"#,
    );

    let output = llmff_cmd()
        .args([
            "plugins",
            "list",
            "--plugin-dir",
            directory.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let plugins: serde_json::Value =
        serde_json::from_slice(&output).expect("plugin list should be valid JSON");

    assert_eq!(plugins[0]["name"], "json-tools");
    assert_eq!(plugins[0]["version"], "0.1.0");
    assert_eq!(plugins[0]["capabilities"][0]["kind"], "stage");
    assert_eq!(plugins[0]["capabilities"][0]["name"], "json.flatten");
}

#[test]
fn plugins_validate_reports_missing_entrypoint_without_pipeline_run() {
    let directory = temp_dir();
    let plugin = directory.path().join("broken-plugin");
    std::fs::create_dir(&plugin).unwrap();
    write_file(
        plugin.join("llmff-plugin.yaml"),
        missing_backend_plugin_manifest(),
    );

    llmff_cmd()
        .args([
            "plugins",
            "validate",
            "--plugin-dir",
            directory.path().to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("missing entrypoint"))
        .stderr(predicate::str::contains("missing-backend"));
}

#[test]
fn plugins_validate_json_reports_missing_entrypoint() {
    let directory = temp_dir();
    let plugin = directory.path().join("broken-plugin");
    std::fs::create_dir(&plugin).unwrap();
    write_file(
        plugin.join("llmff-plugin.yaml"),
        missing_backend_plugin_manifest(),
    );

    let output = llmff_cmd()
        .args([
            "plugins",
            "validate",
            "--plugin-dir",
            directory.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value =
        serde_json::from_slice(&output).expect("plugin validation should be valid JSON");

    assert_eq!(report["valid"], false);
    assert_eq!(report["plugin_count"], 1);
    assert_eq!(report["diagnostics"][0]["code"], "missing_entrypoint");
    assert_eq!(report["diagnostics"][0]["severity"], "error");
    assert_eq!(report["diagnostics"][0]["plugin_name"], "broken-plugin");
    assert_eq!(report["diagnostics"][0]["capability_kind"], "backend");
    assert_eq!(
        report["diagnostics"][0]["capability_name"],
        "missing-backend"
    );
    assert!(report["diagnostics"][0]["entrypoint"]
        .as_str()
        .unwrap()
        .ends_with("broken-plugin/./bin/missing-backend"));
}

#[test]
fn plugins_validate_json_reports_conformance_checks_without_pipeline_run() {
    let directory = temp_dir();
    let plugin = directory.path().join("conformance-plugin");
    let bin = plugin.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::write(bin.join("stage"), "#!/usr/bin/env sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(bin.join("stage"), std::fs::Permissions::from_mode(0o755))
            .unwrap();
    }
    write_file(
        plugin.join("llmff-plugin.yaml"),
        r#"
name: conformance-plugin
version: 0.1.0
capabilities:
  - kind: stage
    name: text.clean
    entrypoint: ./bin/stage
"#,
    );

    let output = llmff_cmd()
        .args([
            "plugins",
            "validate",
            "--plugin-dir",
            directory.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value =
        serde_json::from_slice(&output).expect("plugin validation should be valid JSON");

    assert_eq!(report["valid"], true);
    assert_eq!(report["diagnostics"], serde_json::json!([]));
    let checks = report["conformance_checks"]
        .as_array()
        .expect("conformance checks should be an array");
    assert!(checks
        .iter()
        .any(|check| { check["code"] == "entrypoint_executable" && check["status"] == "passed" }));
    assert!(checks
        .iter()
        .any(|check| { check["code"] == "schema_output_contract" && check["status"] == "passed" }));
    assert!(checks.iter().any(|check| {
        check["code"] == "error_handling_contract" && check["status"] == "passed"
    }));
    assert!(checks
        .iter()
        .any(|check| { check["code"] == "trust_boundary_review" && check["status"] == "warning" }));
}

#[test]
fn plugins_validate_json_rejects_non_executable_entrypoint() {
    let directory = temp_dir();
    let plugin = directory.path().join("broken-plugin");
    let bin = plugin.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::write(bin.join("stage"), "#!/usr/bin/env sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(bin.join("stage"), std::fs::Permissions::from_mode(0o644))
            .unwrap();
    }
    write_file(
        plugin.join("llmff-plugin.yaml"),
        non_executable_stage_plugin_manifest(),
    );

    let output = llmff_cmd()
        .args([
            "plugins",
            "validate",
            "--plugin-dir",
            directory.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value =
        serde_json::from_slice(&output).expect("plugin validation should be valid JSON");

    assert_eq!(report["valid"], false);
    assert_eq!(
        report["diagnostics"][0]["code"],
        "entrypoint_not_executable"
    );
    assert_eq!(report["diagnostics"][0]["plugin_name"], "broken-plugin");
    assert!(report["conformance_checks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|check| check["code"] == "entrypoint_executable" && check["status"] == "error"));
}

#[test]
fn plugins_validate_reports_non_executable_entrypoint_without_pipeline_run() {
    let directory = temp_dir();
    let plugin = directory.path().join("broken-plugin");
    let bin = plugin.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::write(bin.join("stage"), "#!/usr/bin/env sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(bin.join("stage"), std::fs::Permissions::from_mode(0o644))
            .unwrap();
    }
    write_file(
        plugin.join("llmff-plugin.yaml"),
        non_executable_stage_plugin_manifest(),
    );

    llmff_cmd()
        .args([
            "plugins",
            "validate",
            "--plugin-dir",
            directory.path().to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("non-executable entrypoint"))
        .stderr(predicate::str::contains("text.clean"));
}

#[test]
fn plugins_validate_reports_malformed_manifest() {
    let directory = temp_dir();
    let plugin = directory.path().join("broken-plugin");
    std::fs::create_dir(&plugin).unwrap();
    write_file(
        plugin.join("llmff-plugin.yaml"),
        "name: [broken\nversion: 0.1.0\n",
    );

    llmff_cmd()
        .args([
            "plugins",
            "validate",
            "--plugin-dir",
            directory.path().to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("failed to parse plugin manifest"))
        .stderr(predicate::str::contains("llmff-plugin.yaml"));
}

#[test]
fn plugins_validate_accepts_example_plugins() {
    let plugin_dir = workspace_root().join("examples/plugins");

    llmff_cmd()
        .args([
            "plugins",
            "validate",
            "--plugin-dir",
            plugin_dir.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("ok"));
}

#[test]
fn plugins_list_covers_example_plugin_capability_kinds() {
    let plugin_dir = workspace_root().join("examples/plugins");

    let output = llmff_cmd()
        .args([
            "plugins",
            "list",
            "--plugin-dir",
            plugin_dir.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let plugins: serde_json::Value =
        serde_json::from_slice(&output).expect("plugin list should be valid JSON");
    let capability_kinds = plugins
        .as_array()
        .expect("plugin list should be an array")
        .iter()
        .flat_map(|plugin| plugin["capabilities"].as_array().unwrap())
        .map(|capability| capability["kind"].as_str().unwrap())
        .collect::<Vec<_>>();

    assert!(capability_kinds.contains(&"stage"));
    assert!(capability_kinds.contains(&"backend"));
    assert!(capability_kinds.contains(&"sampler"));
    assert!(capability_kinds.contains(&"tool-transport"));
}
