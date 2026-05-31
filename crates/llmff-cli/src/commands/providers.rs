use std::path::PathBuf;

use anyhow::Result;
use llmff_core::backend::builtin_backend_families;
use llmff_core::plugin::discover_plugin_backends;
use llmff_core::stage::builtin_stage_metadata;

use super::{parse_alias_value_list, parse_alias_value_map, OutputFormat};

pub(super) fn inspect_backend_registrations(
    backend: &[String],
    ollama: &[String],
    plugin_dirs: impl IntoIterator<Item = PathBuf>,
) -> Result<Vec<serde_json::Value>> {
    let mut registrations = builtin_backend_families()
        .iter()
        .map(|backend| {
            serde_json::json!({
                "name": backend.name,
                "kind": backend.kind,
                "source": "built-in",
                "registration_flag": backend.registration_flag,
                "base_url": null,
                "requires_api_key": backend.requires_api_key,
                "model_aliases": backend.model_aliases,
                "capabilities": backend.capabilities,
            })
        })
        .collect::<Vec<_>>();

    for backend in parse_alias_value_list(backend.to_vec())? {
        registrations.push(serde_json::json!({
            "name": backend.alias,
            "kind": "openai-compatible",
            "source": "cli",
            "registration_flag": format!("--backend {}=<base-url>", backend.alias),
            "base_url": backend.value,
            "requires_api_key": true,
            "model_aliases": [format!("{}:<model>", backend.alias)],
            "capabilities": [
                "chat-messages",
                "response-format-json",
                "sampling",
                "seed-control",
                "stop-sequences",
                "streaming-inference",
                "usage-metadata",
            ],
        }));
    }

    for backend in parse_alias_value_list(ollama.to_vec())? {
        registrations.push(serde_json::json!({
            "name": backend.alias,
            "kind": "ollama",
            "source": "cli",
            "registration_flag": format!("--ollama {}=<base-url>", backend.alias),
            "base_url": backend.value,
            "requires_api_key": false,
            "model_aliases": [format!("{}:<model>", backend.alias)],
            "capabilities": [
                "chat-messages",
                "response-format-json",
                "sampling",
                "seed-control",
                "stop-sequences",
                "usage-metadata",
            ],
        }));
    }

    for plugin_dir in plugin_dirs {
        for backend in discover_plugin_backends(&plugin_dir)? {
            registrations.push(serde_json::json!({
                "name": backend.name,
                "kind": "plugin-command",
                "source": "plugin",
                "registration_flag": "--plugin-dir",
                "base_url": null,
                "requires_api_key": false,
                "model_aliases": [format!("{}:<model>", backend.name)],
                "capabilities": [
                    "chat-messages",
                    "command-backend",
                    "usage-metadata",
                ],
            }));
        }
    }

    Ok(registrations)
}

pub(super) fn print_backend_report(
    format: OutputFormat,
    backend: Vec<String>,
    ollama: Vec<String>,
    api_key_env: Vec<String>,
    api_key: Vec<String>,
    plugin_dir: Vec<PathBuf>,
) -> Result<()> {
    let report = backend_report_views(backend, ollama, api_key_env, api_key, plugin_dir)?;
    match format {
        OutputFormat::Text => {
            for backend in report {
                let name = backend["name"].as_str().unwrap_or("unknown");
                let kind = backend["kind"].as_str().unwrap_or("unknown");
                println!("{name} ({kind})");
                for capability in ["json_mode", "streaming", "seed", "stop", "usage_metadata"] {
                    let supported = backend["capabilities"][capability]["supported"]
                        .as_bool()
                        .unwrap_or(false);
                    println!("  {capability}: {supported}");
                }
                if let Some(diagnostics) = backend["diagnostics"].as_array() {
                    for diagnostic in diagnostics {
                        if let Some(message) = diagnostic["message"].as_str() {
                            println!("  diagnostic: {message}");
                        }
                    }
                }
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
    }

    Ok(())
}

fn backend_report_views(
    backend: Vec<String>,
    ollama: Vec<String>,
    api_key_env: Vec<String>,
    api_key: Vec<String>,
    plugin_dir: Vec<PathBuf>,
) -> Result<Vec<serde_json::Value>> {
    let api_key_env = parse_alias_value_map(api_key_env)?;
    let api_key = parse_alias_value_map(api_key)?;
    let mut report = Vec::new();

    for family in builtin_backend_families() {
        let aliases = if family.model_aliases.is_empty() {
            vec![family.name.to_string()]
        } else {
            family
                .model_aliases
                .iter()
                .map(|alias| (*alias).to_string())
                .collect()
        };
        report.push(provider_report(
            family.name,
            family.kind,
            "built-in",
            None,
            family.requires_api_key,
            false,
            aliases,
        ));
    }

    for backend in parse_alias_value_list(backend)? {
        let key_configured =
            api_key.contains_key(&backend.alias) || api_key_env.contains_key(&backend.alias);
        report.push(provider_report(
            &backend.alias,
            "openai-compatible",
            "cli",
            Some(&backend.value),
            true,
            key_configured,
            vec![format!("{}:<model>", backend.alias)],
        ));
    }

    for backend in parse_alias_value_list(ollama)? {
        report.push(provider_report(
            &backend.alias,
            "ollama",
            "cli",
            Some(&backend.value),
            false,
            false,
            vec![format!("{}:<model>", backend.alias)],
        ));
    }

    for plugin_dir in plugin_dir {
        for backend in discover_plugin_backends(&plugin_dir)? {
            report.push(provider_report(
                &backend.name,
                "plugin-command",
                "plugin",
                None,
                false,
                false,
                vec![format!("{}:<model>", backend.name)],
            ));
        }
    }

    Ok(report)
}

fn provider_report(
    name: &str,
    kind: &str,
    source: &str,
    base_url: Option<&str>,
    requires_api_key: bool,
    api_key_configured: bool,
    model_aliases: Vec<String>,
) -> serde_json::Value {
    let mut diagnostics = Vec::new();
    if requires_api_key && !api_key_configured && source == "cli" {
        diagnostics.push(serde_json::json!({
            "severity": "warning",
            "code": "api_key_missing",
            "message": format!("backend `{name}` requires an API key; configure --api-key-env {name}=ENV_NAME or --api-key {name}=VALUE"),
        }));
    }
    if kind == "ollama" {
        diagnostics.push(serde_json::json!({
            "severity": "info",
            "code": "streaming_not_supported",
            "message": "llmff uses Ollama through the non-streaming chat path",
        }));
    }

    serde_json::json!({
        "name": name,
        "kind": kind,
        "source": source,
        "base_url": base_url,
        "requires_api_key": requires_api_key,
        "api_key_configured": api_key_configured,
        "model_aliases": model_aliases,
        "capabilities": provider_capabilities(kind),
        "diagnostics": diagnostics,
    })
}

fn provider_capabilities(kind: &str) -> serde_json::Value {
    match kind {
        "remote-chat" | "openai-compatible" => serde_json::json!({
            "json_mode": capability(true, "response_format json_object"),
            "streaming": capability(true, "server-sent chat completion chunks"),
            "seed": capability(true, "seed request field"),
            "stop": capability(true, "stop request field"),
            "usage_metadata": capability(true, "usage response field"),
        }),
        "local-chat" | "ollama" => serde_json::json!({
            "json_mode": capability(true, "format json"),
            "streaming": capability(false, "Ollama streaming is not wired through llmff yet"),
            "seed": capability(true, "options.seed request field"),
            "stop": capability(true, "options.stop request field"),
            "usage_metadata": capability(true, "prompt_eval_count and eval_count response fields"),
        }),
        "plugin-command" => serde_json::json!({
            "json_mode": capability(false, "plugin-specific"),
            "streaming": capability(false, "command backends are request-response"),
            "seed": capability(false, "plugin-specific"),
            "stop": capability(false, "plugin-specific"),
            "usage_metadata": capability(true, "optional usage response field"),
        }),
        _ => serde_json::json!({
            "json_mode": capability(false, "not applicable"),
            "streaming": capability(false, "not applicable"),
            "seed": capability(false, "not applicable"),
            "stop": capability(false, "not applicable"),
            "usage_metadata": capability(false, "not applicable"),
        }),
    }
}

fn capability(supported: bool, detail: &str) -> serde_json::Value {
    serde_json::json!({
        "supported": supported,
        "detail": detail,
    })
}

pub(super) fn print_stage_metadata(format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Text => {
            for stage in builtin_stage_metadata() {
                println!("{}", stage.name);
            }
        }
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(builtin_stage_metadata())?
            );
        }
    }

    Ok(())
}

pub(super) fn print_backend_families(
    format: OutputFormat,
    backend: Vec<String>,
    ollama: Vec<String>,
    plugin_dir: Vec<PathBuf>,
) -> Result<()> {
    let backends = backend_family_views(backend, ollama, plugin_dir)?;
    match format {
        OutputFormat::Text => {
            for backend in backends {
                let model_aliases = backend
                    .get("model_aliases")
                    .and_then(serde_json::Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                if model_aliases.is_empty() {
                    if let Some(name) = backend.get("name").and_then(serde_json::Value::as_str) {
                        println!("{name}");
                    }
                } else {
                    for alias in model_aliases {
                        if let Some(alias) = alias.as_str() {
                            println!("{alias}");
                        }
                    }
                }
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&backends)?);
        }
    }

    Ok(())
}

fn backend_family_views(
    backend: Vec<String>,
    ollama: Vec<String>,
    plugin_dir: Vec<PathBuf>,
) -> Result<Vec<serde_json::Value>> {
    let mut families = builtin_backend_families()
        .iter()
        .map(|backend| {
            serde_json::json!({
                "name": backend.name,
                "kind": backend.kind,
                "registration_flag": backend.registration_flag,
                "requires_api_key": backend.requires_api_key,
                "model_aliases": backend.model_aliases,
                "capabilities": backend.capabilities,
            })
        })
        .collect::<Vec<_>>();

    for backend in parse_alias_value_list(backend)? {
        families.push(serde_json::json!({
            "name": backend.alias,
            "kind": "openai-compatible",
            "registration_flag": format!("--backend {}=<base-url>", backend.alias),
            "requires_api_key": true,
            "model_aliases": [format!("{}:<model>", backend.alias)],
            "capabilities": [
                "chat-messages",
                "response-format-json",
                "sampling",
                "seed-control",
                "stop-sequences",
                "streaming-inference",
                "usage-metadata",
            ],
        }));
    }

    for backend in parse_alias_value_list(ollama)? {
        families.push(serde_json::json!({
            "name": backend.alias,
            "kind": "ollama",
            "registration_flag": format!("--ollama {}=<base-url>", backend.alias),
            "requires_api_key": false,
            "model_aliases": [format!("{}:<model>", backend.alias)],
            "capabilities": [
                "chat-messages",
                "response-format-json",
                "sampling",
                "seed-control",
                "stop-sequences",
                "usage-metadata",
            ],
        }));
    }

    for plugin_dir in plugin_dir {
        for backend in discover_plugin_backends(&plugin_dir)? {
            families.push(serde_json::json!({
                "name": backend.name,
                "kind": "plugin-command",
                "registration_flag": "--plugin-dir",
                "requires_api_key": false,
                "model_aliases": [format!("{}:<model>", backend.name)],
                "capabilities": [
                    "chat-messages",
                    "command-backend",
                    "usage-metadata",
                ],
            }));
        }
    }

    Ok(families)
}

pub(super) fn print_model_runtimes(
    format: OutputFormat,
    backend: Vec<String>,
    ollama: Vec<String>,
    plugin_dir: Vec<PathBuf>,
) -> Result<()> {
    let models = model_runtime_views(backend, ollama, plugin_dir)?;
    match format {
        OutputFormat::Text => {
            for model in models {
                if let Some(name) = model.get("model").and_then(serde_json::Value::as_str) {
                    println!("{name}");
                }
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&models)?);
        }
    }

    Ok(())
}

fn model_runtime_views(
    backend: Vec<String>,
    ollama: Vec<String>,
    plugin_dir: Vec<PathBuf>,
) -> Result<Vec<serde_json::Value>> {
    let mut models = Vec::new();

    for family in builtin_backend_families() {
        for model in family.model_aliases {
            models.push(serde_json::json!({
                "model": model,
                "backend": family.name,
                "backend_kind": family.kind,
                "runtime": family.kind,
                "source": "built-in",
                "requires_api_key": family.requires_api_key,
                "registration_flag": family.registration_flag,
                "capabilities": family.capabilities,
            }));
        }
    }

    for backend in parse_alias_value_list(backend)? {
        models.push(serde_json::json!({
            "model": format!("{}:<model>", backend.alias),
            "backend": backend.alias,
            "backend_kind": "openai-compatible",
            "runtime": "remote-chat",
            "source": "cli",
            "requires_api_key": true,
            "registration_flag": format!("--backend {}=<base-url>", backend.alias),
            "capabilities": [
                "chat-messages",
                "response-format-json",
                "sampling",
                "seed-control",
                "stop-sequences",
                "streaming-inference",
                "usage-metadata",
            ],
        }));
    }

    for backend in parse_alias_value_list(ollama)? {
        models.push(serde_json::json!({
            "model": format!("{}:<model>", backend.alias),
            "backend": backend.alias,
            "backend_kind": "ollama",
            "runtime": "local-chat",
            "source": "cli",
            "requires_api_key": false,
            "registration_flag": format!("--ollama {}=<base-url>", backend.alias),
            "capabilities": [
                "chat-messages",
                "response-format-json",
                "sampling",
                "seed-control",
                "stop-sequences",
                "usage-metadata",
            ],
        }));
    }

    for plugin_dir in plugin_dir {
        for backend in discover_plugin_backends(&plugin_dir)? {
            models.push(serde_json::json!({
                "model": format!("{}:<model>", backend.name),
                "backend": backend.name,
                "backend_kind": "plugin-command",
                "runtime": "command",
                "source": "plugin",
                "requires_api_key": false,
                "registration_flag": "--plugin-dir",
                "capabilities": [
                    "chat-messages",
                    "command-backend",
                    "usage-metadata",
                ],
            }));
        }
    }

    Ok(models)
}
