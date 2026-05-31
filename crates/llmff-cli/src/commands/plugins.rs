use std::path::{Path, PathBuf};

use anyhow::Result;
use llmff_core::plugin::{discover_plugin_manifests, validate_plugin_directory};

use super::OutputFormat;

pub(super) fn inspect_plugin_manifests(
    plugin_dirs: impl IntoIterator<Item = PathBuf>,
) -> Result<Vec<serde_json::Value>> {
    let mut manifests = Vec::new();
    for plugin_dir in plugin_dirs {
        for manifest in discover_plugin_manifests(&plugin_dir)? {
            manifests.push(serde_json::json!({
                "name": manifest.name,
                "version": manifest.version,
                "capabilities": manifest.capabilities,
            }));
        }
    }
    Ok(manifests)
}

pub(super) fn print_plugin_manifests(plugin_dir: &Path, format: OutputFormat) -> Result<()> {
    let manifests = discover_plugin_manifests(plugin_dir)?;
    match format {
        OutputFormat::Text => {
            for manifest in manifests {
                println!("{} {}", manifest.name, manifest.version);
                for capability in manifest.capabilities {
                    println!(
                        "  {} {} {}",
                        capability.kind, capability.name, capability.entrypoint
                    );
                }
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&manifests)?);
        }
    }

    Ok(())
}

pub(super) fn validate_plugins(plugin_dir: &Path, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Text => {
            let report = validate_plugin_directory(plugin_dir)?;
            if !report.valid {
                let messages = report
                    .diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.message.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                anyhow::bail!("{messages}");
            }
            println!("ok");
        }
        OutputFormat::Json => {
            let report = validate_plugin_directory(plugin_dir)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            if !report.valid {
                anyhow::bail!("plugin validation failed");
            }
        }
    }

    Ok(())
}
