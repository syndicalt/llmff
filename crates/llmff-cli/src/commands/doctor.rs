use std::path::{Path, PathBuf};

use anyhow::Result;
use llmff_core::plugin::validate_plugin_directory;

use super::{parse_alias_value_list, parse_alias_value_map};

pub(super) struct DoctorOptions {
    pub(super) run_dir: Option<PathBuf>,
    pub(super) plugin_dir: Vec<PathBuf>,
    pub(super) backend: Vec<String>,
    pub(super) api_key_env: Vec<String>,
    pub(super) release_manifest: Option<PathBuf>,
}

pub(super) fn run_doctor(options: DoctorOptions) -> Result<()> {
    println!("version {} ok", env!("CARGO_PKG_VERSION"));

    if let Some(run_dir) = options.run_dir.as_ref() {
        check_writable_run_dir(run_dir)?;
        println!("run-dir {} writable", run_dir.display());
    }

    for plugin_dir in &options.plugin_dir {
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
        println!("plugin-dir {} ok", plugin_dir.display());
    }

    let api_key_env = parse_alias_value_map(options.api_key_env)?;
    for backend in parse_alias_value_list(options.backend)? {
        if let Some(env_name) = api_key_env.get(&backend.alias) {
            std::env::var(env_name).map_err(|_| {
                anyhow::anyhow!(
                    "api key env `{env_name}` for backend `{}` is not set",
                    backend.alias
                )
            })?;
            println!("api-key-env {}={} set", backend.alias, env_name);
        } else {
            println!("api-key-env {} not configured", backend.alias);
        }
    }

    if let Some(release_manifest) = options.release_manifest.as_ref() {
        if !release_manifest.exists() {
            anyhow::bail!(
                "release trust manifest `{}` does not exist",
                release_manifest.display()
            );
        }
        println!("release-manifest {} present", release_manifest.display());
    }

    Ok(())
}

fn check_writable_run_dir(run_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(run_dir)?;
    let probe = run_dir.join(".llmff-doctor-write-test");
    std::fs::write(&probe, b"ok")?;
    std::fs::remove_file(probe)?;
    Ok(())
}
