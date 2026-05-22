use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::LlmffError;

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

        let source = std::fs::read_to_string(&manifest_path).map_err(|error| {
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
        manifest.validate(&manifest_path)?;
        manifests.push(manifest);
    }

    manifests.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(manifests)
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
}
