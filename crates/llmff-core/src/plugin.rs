use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::LlmffError;

pub const PLUGIN_PROTOCOL_VERSION: u32 = 1;
pub const PLUGIN_VALIDATION_REPORT_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub capabilities: Vec<PluginCapability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginCapability {
    pub kind: String,
    pub name: String,
    pub entrypoint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginToolTransport {
    pub name: String,
    pub entrypoint: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginStage {
    pub name: String,
    pub entrypoint: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginSampler {
    pub name: String,
    pub entrypoint: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginBackend {
    pub name: String,
    pub entrypoint: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginValidationReport {
    pub format_version: u32,
    pub plugin_protocol_version: u32,
    pub plugin_dir: String,
    pub valid: bool,
    pub plugin_count: usize,
    pub plugins: Vec<PluginManifest>,
    pub diagnostics: Vec<PluginDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginDiagnostic {
    pub severity: PluginDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub manifest_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capability_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capability_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginDiagnosticSeverity {
    Error,
}

pub fn discover_plugin_manifests(
    directory: impl AsRef<Path>,
) -> Result<Vec<PluginManifest>, LlmffError> {
    let directory = directory.as_ref();
    let entries = std::fs::read_dir(directory).map_err(|error| {
        LlmffError::Config(format!(
            "failed to read plugin directory `{}`: {error}",
            directory.display()
        ))
    })?;
    let mut manifests = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|error| {
            LlmffError::Config(format!(
                "failed to read plugin directory entry in `{}`: {error}",
                directory.display()
            ))
        })?;
        let manifest_path = entry.path().join("llmff-plugin.yaml");
        if !manifest_path.is_file() {
            continue;
        }

        manifests.push(read_plugin_manifest(&manifest_path)?);
    }

    manifests.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(manifests)
}

pub fn validate_plugin_directory(
    directory: impl AsRef<Path>,
) -> Result<PluginValidationReport, LlmffError> {
    let directory = directory.as_ref();
    let entries = std::fs::read_dir(directory).map_err(|error| {
        LlmffError::Config(format!(
            "failed to read plugin directory `{}`: {error}",
            directory.display()
        ))
    })?;
    let mut plugins = Vec::new();
    let mut diagnostics = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|error| {
            LlmffError::Config(format!(
                "failed to read plugin directory entry in `{}`: {error}",
                directory.display()
            ))
        })?;
        let plugin_root = entry.path();
        let manifest_path = plugin_root.join("llmff-plugin.yaml");
        if !manifest_path.is_file() {
            continue;
        }

        match read_plugin_manifest_for_report(&manifest_path) {
            Ok(manifest) => {
                for capability in &manifest.capabilities {
                    let entrypoint = resolve_entrypoint(&plugin_root, &capability.entrypoint);
                    if !entrypoint.is_file() {
                        diagnostics.push(missing_entrypoint_diagnostic(
                            &manifest_path,
                            &manifest,
                            capability,
                            &entrypoint,
                        ));
                    }
                }
                plugins.push(manifest);
            }
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }

    plugins.sort_by(|left, right| left.name.cmp(&right.name));
    diagnostics.sort_by(|left, right| {
        left.manifest_path
            .cmp(&right.manifest_path)
            .then_with(|| left.code.cmp(&right.code))
            .then_with(|| left.capability_name.cmp(&right.capability_name))
    });

    Ok(PluginValidationReport {
        format_version: PLUGIN_VALIDATION_REPORT_FORMAT_VERSION,
        plugin_protocol_version: PLUGIN_PROTOCOL_VERSION,
        plugin_dir: directory.display().to_string(),
        valid: diagnostics.is_empty(),
        plugin_count: plugins.len(),
        plugins,
        diagnostics,
    })
}

pub fn validate_plugin_manifests(
    directory: impl AsRef<Path>,
) -> Result<Vec<PluginManifest>, LlmffError> {
    let directory = directory.as_ref();
    let entries = std::fs::read_dir(directory).map_err(|error| {
        LlmffError::Config(format!(
            "failed to read plugin directory `{}`: {error}",
            directory.display()
        ))
    })?;
    let mut manifests = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|error| {
            LlmffError::Config(format!(
                "failed to read plugin directory entry in `{}`: {error}",
                directory.display()
            ))
        })?;
        let plugin_root = entry.path();
        let manifest_path = plugin_root.join("llmff-plugin.yaml");
        if !manifest_path.is_file() {
            continue;
        }

        let manifest = read_plugin_manifest(&manifest_path)?;
        for capability in &manifest.capabilities {
            let entrypoint = resolve_entrypoint(&plugin_root, &capability.entrypoint);
            if !entrypoint.is_file() {
                return Err(LlmffError::Config(format!(
                    "plugin manifest `{}` capability `{}` `{}` has missing entrypoint `{}`",
                    manifest_path.display(),
                    capability.kind,
                    capability.name,
                    entrypoint.display()
                )));
            }
        }
        manifests.push(manifest);
    }

    manifests.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(manifests)
}

pub fn discover_plugin_tool_transports(
    directory: impl AsRef<Path>,
) -> Result<Vec<PluginToolTransport>, LlmffError> {
    let directory = directory.as_ref();
    let mut transports = Vec::new();

    for plugin_capability in discover_plugin_capabilities(directory, "tool-transport")? {
        transports.push(PluginToolTransport {
            name: plugin_capability.name,
            entrypoint: plugin_capability.entrypoint,
        });
    }

    transports.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(transports)
}

pub fn discover_plugin_stages(directory: impl AsRef<Path>) -> Result<Vec<PluginStage>, LlmffError> {
    let directory = directory.as_ref();
    let mut stages = Vec::new();

    for plugin_capability in discover_plugin_capabilities(directory, "stage")? {
        stages.push(PluginStage {
            name: plugin_capability.name,
            entrypoint: plugin_capability.entrypoint,
        });
    }

    stages.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(stages)
}

pub fn discover_plugin_samplers(
    directory: impl AsRef<Path>,
) -> Result<Vec<PluginSampler>, LlmffError> {
    let directory = directory.as_ref();
    let mut samplers = Vec::new();

    for plugin_capability in discover_plugin_capabilities(directory, "sampler")? {
        samplers.push(PluginSampler {
            name: plugin_capability.name,
            entrypoint: plugin_capability.entrypoint,
        });
    }

    samplers.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(samplers)
}

pub fn discover_plugin_backends(
    directory: impl AsRef<Path>,
) -> Result<Vec<PluginBackend>, LlmffError> {
    let directory = directory.as_ref();
    let mut backends = Vec::new();

    for plugin_capability in discover_plugin_capabilities(directory, "backend")? {
        backends.push(PluginBackend {
            name: plugin_capability.name,
            entrypoint: plugin_capability.entrypoint,
        });
    }

    backends.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(backends)
}

impl PluginManifest {
    fn validate(&self, path: &Path) -> Result<(), LlmffError> {
        require_non_empty("name", &self.name, path)?;
        require_non_empty("version", &self.version, path)?;
        if self.capabilities.is_empty() {
            return Err(LlmffError::Config(format!(
                "plugin manifest `{}` must declare at least one capability",
                path.display()
            )));
        }
        for capability in &self.capabilities {
            capability.validate(path)?;
        }
        Ok(())
    }
}

impl PluginCapability {
    fn validate(&self, path: &Path) -> Result<(), LlmffError> {
        require_non_empty("capabilities.kind", &self.kind, path)?;
        require_non_empty("capabilities.name", &self.name, path)?;
        require_non_empty("capabilities.entrypoint", &self.entrypoint, path)?;
        match self.kind.as_str() {
            "backend" | "sampler" | "stage" | "tool-transport" => Ok(()),
            kind => Err(LlmffError::Config(format!(
                "plugin manifest `{}` has unknown capability kind `{kind}`",
                path.display()
            ))),
        }
    }
}

fn require_non_empty(field: &str, value: &str, path: &Path) -> Result<(), LlmffError> {
    if value.trim().is_empty() {
        return Err(LlmffError::Config(format!(
            "plugin manifest `{}` has empty `{field}`",
            path.display()
        )));
    }
    Ok(())
}

fn read_plugin_manifest(manifest_path: &Path) -> Result<PluginManifest, LlmffError> {
    let source = std::fs::read_to_string(manifest_path).map_err(|error| {
        LlmffError::Config(format!(
            "failed to read plugin manifest `{}`: {error}",
            manifest_path.display()
        ))
    })?;
    let manifest: PluginManifest = serde_yaml::from_str(&source).map_err(|error| {
        LlmffError::Config(format!(
            "failed to parse plugin manifest `{}`: {error}",
            manifest_path.display()
        ))
    })?;
    manifest.validate(manifest_path)?;
    Ok(manifest)
}

fn read_plugin_manifest_for_report(
    manifest_path: &Path,
) -> Result<PluginManifest, PluginDiagnostic> {
    let source = std::fs::read_to_string(manifest_path).map_err(|error| PluginDiagnostic {
        severity: PluginDiagnosticSeverity::Error,
        code: "manifest_read_failed".to_string(),
        message: format!(
            "failed to read plugin manifest `{}`: {error}",
            manifest_path.display()
        ),
        manifest_path: manifest_path.display().to_string(),
        plugin_name: None,
        capability_kind: None,
        capability_name: None,
        entrypoint: None,
    })?;
    let manifest: PluginManifest =
        serde_yaml::from_str(&source).map_err(|error| PluginDiagnostic {
            severity: PluginDiagnosticSeverity::Error,
            code: "manifest_parse_failed".to_string(),
            message: format!(
                "failed to parse plugin manifest `{}`: {error}",
                manifest_path.display()
            ),
            manifest_path: manifest_path.display().to_string(),
            plugin_name: None,
            capability_kind: None,
            capability_name: None,
            entrypoint: None,
        })?;
    manifest
        .validate(manifest_path)
        .map_err(|error| PluginDiagnostic {
            severity: PluginDiagnosticSeverity::Error,
            code: "manifest_invalid".to_string(),
            message: error.to_string(),
            manifest_path: manifest_path.display().to_string(),
            plugin_name: Some(manifest.name.clone()),
            capability_kind: None,
            capability_name: None,
            entrypoint: None,
        })?;
    Ok(manifest)
}

fn missing_entrypoint_diagnostic(
    manifest_path: &Path,
    manifest: &PluginManifest,
    capability: &PluginCapability,
    entrypoint: &Path,
) -> PluginDiagnostic {
    PluginDiagnostic {
        severity: PluginDiagnosticSeverity::Error,
        code: "missing_entrypoint".to_string(),
        message: format!(
            "plugin manifest `{}` capability `{}` `{}` has missing entrypoint `{}`",
            manifest_path.display(),
            capability.kind,
            capability.name,
            entrypoint.display()
        ),
        manifest_path: manifest_path.display().to_string(),
        plugin_name: Some(manifest.name.clone()),
        capability_kind: Some(capability.kind.clone()),
        capability_name: Some(capability.name.clone()),
        entrypoint: Some(entrypoint.display().to_string()),
    }
}

fn resolve_entrypoint(plugin_root: &Path, entrypoint: &str) -> PathBuf {
    let entrypoint = PathBuf::from(entrypoint);
    if entrypoint.is_absolute() {
        entrypoint
    } else {
        plugin_root.join(entrypoint)
    }
}

struct ResolvedPluginCapability {
    name: String,
    entrypoint: PathBuf,
}

fn discover_plugin_capabilities(
    directory: &Path,
    kind: &str,
) -> Result<Vec<ResolvedPluginCapability>, LlmffError> {
    let entries = std::fs::read_dir(directory).map_err(|error| {
        LlmffError::Config(format!(
            "failed to read plugin directory `{}`: {error}",
            directory.display()
        ))
    })?;
    let mut capabilities = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|error| {
            LlmffError::Config(format!(
                "failed to read plugin directory entry in `{}`: {error}",
                directory.display()
            ))
        })?;
        let plugin_root = entry.path();
        let manifest_path = plugin_root.join("llmff-plugin.yaml");
        if !manifest_path.is_file() {
            continue;
        }
        let manifest = read_plugin_manifest(&manifest_path)?;

        for capability in manifest.capabilities {
            if capability.kind != kind {
                continue;
            }
            let entrypoint = resolve_entrypoint(&plugin_root, &capability.entrypoint);
            capabilities.push(ResolvedPluginCapability {
                name: capability.name,
                entrypoint,
            });
        }
    }

    Ok(capabilities)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_plugin_manifests_from_directory() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join("json-tools")).unwrap();
        std::fs::write(
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
  - kind: tool-transport
    name: stdio-json
    entrypoint: ./stdio_json
"#,
        )
        .unwrap();

        let manifests = discover_plugin_manifests(directory.path()).unwrap();

        assert_eq!(manifests.len(), 1);
        assert_eq!(manifests[0].name, "json-tools");
        assert_eq!(manifests[0].version, "0.1.0");
        assert_eq!(
            manifests[0].capabilities,
            vec![
                PluginCapability {
                    kind: "stage".to_string(),
                    name: "json.flatten".to_string(),
                    entrypoint: "./json_flatten".to_string(),
                },
                PluginCapability {
                    kind: "tool-transport".to_string(),
                    name: "stdio-json".to_string(),
                    entrypoint: "./stdio_json".to_string(),
                },
            ]
        );
    }

    #[test]
    fn rejects_unknown_plugin_capability_kind() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join("bad")).unwrap();
        std::fs::write(
            directory.path().join("bad").join("llmff-plugin.yaml"),
            r#"
name: bad
version: 0.1.0
capabilities:
  - kind: widget
    name: nope
    entrypoint: ./nope
"#,
        )
        .unwrap();

        let error = discover_plugin_manifests(directory.path()).unwrap_err();

        assert!(error
            .to_string()
            .contains("unknown capability kind `widget`"));
    }

    #[test]
    fn validation_reports_missing_entrypoints() {
        let directory = tempfile::tempdir().unwrap();
        let plugin_root = directory.path().join("bad");
        std::fs::create_dir(&plugin_root).unwrap();
        std::fs::write(
            plugin_root.join("llmff-plugin.yaml"),
            r#"
name: bad
version: 0.1.0
capabilities:
  - kind: stage
    name: missing.stage
    entrypoint: ./bin/missing
"#,
        )
        .unwrap();

        let error = validate_plugin_manifests(directory.path()).unwrap_err();

        assert!(error.to_string().contains("missing entrypoint"));
        assert!(error.to_string().contains("missing.stage"));
        assert!(error.to_string().contains("bad/./bin/missing"));
    }

    #[test]
    fn validation_report_contains_structured_missing_entrypoint_diagnostic() {
        let directory = tempfile::tempdir().unwrap();
        let plugin_root = directory.path().join("bad");
        std::fs::create_dir(&plugin_root).unwrap();
        std::fs::write(
            plugin_root.join("llmff-plugin.yaml"),
            r#"
name: bad
version: 0.1.0
capabilities:
  - kind: stage
    name: missing.stage
    entrypoint: ./bin/missing
"#,
        )
        .unwrap();

        let report = validate_plugin_directory(directory.path()).unwrap();

        assert!(!report.valid);
        assert_eq!(report.plugin_count, 1);
        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(report.diagnostics[0].code, "missing_entrypoint");
        assert_eq!(
            report.diagnostics[0].severity,
            PluginDiagnosticSeverity::Error
        );
        assert_eq!(report.diagnostics[0].plugin_name.as_deref(), Some("bad"));
        assert_eq!(
            report.diagnostics[0].capability_name.as_deref(),
            Some("missing.stage")
        );
        assert_eq!(
            report.diagnostics[0].capability_kind.as_deref(),
            Some("stage")
        );
        assert!(report.diagnostics[0]
            .entrypoint
            .as_ref()
            .unwrap()
            .ends_with("bad/./bin/missing"));
    }

    #[test]
    fn discovers_tool_transports_with_entrypoints_relative_to_plugin_root() {
        let directory = tempfile::tempdir().unwrap();
        let plugin_root = directory.path().join("json-tools");
        std::fs::create_dir(&plugin_root).unwrap();
        std::fs::write(
            plugin_root.join("llmff-plugin.yaml"),
            r#"
name: json-tools
version: 0.1.0
capabilities:
  - kind: tool-transport
    name: stdio-json
    entrypoint: ./bin/stdio-json
"#,
        )
        .unwrap();

        let transports = discover_plugin_tool_transports(directory.path()).unwrap();

        assert_eq!(
            transports,
            vec![PluginToolTransport {
                name: "stdio-json".to_string(),
                entrypoint: plugin_root.join("./bin/stdio-json"),
            }]
        );
    }

    #[test]
    fn discovers_plugin_stages_with_entrypoints_relative_to_plugin_root() {
        let directory = tempfile::tempdir().unwrap();
        let plugin_root = directory.path().join("json-tools");
        std::fs::create_dir(&plugin_root).unwrap();
        std::fs::write(
            plugin_root.join("llmff-plugin.yaml"),
            r#"
name: json-tools
version: 0.1.0
capabilities:
  - kind: stage
    name: json.uppercase
    entrypoint: ./bin/uppercase
"#,
        )
        .unwrap();

        let stages = discover_plugin_stages(directory.path()).unwrap();

        assert_eq!(
            stages,
            vec![PluginStage {
                name: "json.uppercase".to_string(),
                entrypoint: plugin_root.join("./bin/uppercase"),
            }]
        );
    }

    #[test]
    fn discovers_plugin_backends_with_entrypoints_relative_to_plugin_root() {
        let directory = tempfile::tempdir().unwrap();
        let plugin_root = directory.path().join("model-tools");
        std::fs::create_dir(&plugin_root).unwrap();
        std::fs::write(
            plugin_root.join("llmff-plugin.yaml"),
            r#"
name: model-tools
version: 0.1.0
capabilities:
  - kind: backend
    name: local-echo
    entrypoint: ./bin/local-echo
"#,
        )
        .unwrap();

        let backends = discover_plugin_backends(directory.path()).unwrap();

        assert_eq!(
            backends,
            vec![PluginBackend {
                name: "local-echo".to_string(),
                entrypoint: plugin_root.join("./bin/local-echo"),
            }]
        );
    }

    #[test]
    fn discovers_plugin_samplers_with_entrypoints_relative_to_plugin_root() {
        let directory = tempfile::tempdir().unwrap();
        let plugin_root = directory.path().join("sampling-tools");
        std::fs::create_dir(&plugin_root).unwrap();
        std::fs::write(
            plugin_root.join("llmff-plugin.yaml"),
            r#"
name: sampling-tools
version: 0.1.0
capabilities:
  - kind: sampler
    name: safe-small
    entrypoint: ./bin/safe-small
"#,
        )
        .unwrap();

        let samplers = discover_plugin_samplers(directory.path()).unwrap();

        assert_eq!(
            samplers,
            vec![PluginSampler {
                name: "safe-small".to_string(),
                entrypoint: plugin_root.join("./bin/safe-small"),
            }]
        );
    }
}
