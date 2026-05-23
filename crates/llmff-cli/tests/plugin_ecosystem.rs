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

#[test]
fn ecosystem_integration_paths_are_readiness_gated() {
    let root = workspace_root();
    let guide = root.join("docs/ecosystem-readiness.md");
    assert!(guide.exists(), "missing ecosystem readiness guide");

    let source = std::fs::read_to_string(&guide).expect("guide should be readable");
    for integration in [
        "Manifest contracts",
        "Trace and event streams",
        "CLI JSON output",
        "Plugin protocol",
        "Provider onboarding",
        "Agent subprocess embedding",
        "Package-manager metadata",
        "Release assets",
    ] {
        assert!(
            source.contains(integration),
            "ecosystem readiness guide should cover {integration}"
        );
    }

    Command::new(root.join("scripts/check-ecosystem-readiness.sh"))
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "ecosystem readiness validation succeeded",
        ));
}

#[test]
fn manifest_reproducibility_policy_is_checked() {
    let root = workspace_root();
    let guide = root.join("docs/manifest-reproducibility.md");
    assert!(guide.exists(), "missing manifest reproducibility guide");

    let source = std::fs::read_to_string(&guide).expect("guide should be readable");
    for required in [
        "manifest hash",
        "resolved inputs",
        "resolved outputs",
        "stage order",
        "backend aliases",
        "model ids",
        "plugin dependencies",
        "cache policy",
        "checkpoint/resume policy",
        "manifest lockfile remains parked",
        "materially improves portability",
    ] {
        assert!(
            source.contains(required),
            "manifest reproducibility guide should cover {required}"
        );
    }

    Command::new(root.join("scripts/check-manifest-reproducibility.sh"))
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "manifest reproducibility validation succeeded",
        ));
}

#[test]
fn apt_repository_publication_requires_signed_metadata_design() {
    let root = workspace_root();
    let design = root.join("docs/apt-repository-design.md");
    assert!(design.exists(), "missing apt repository design");

    let source = std::fs::read_to_string(&design).expect("design should be readable");
    for required in [
        "signed repository metadata",
        "InRelease",
        "Release.gpg",
        "key rotation",
        "historical retention",
        "hosting",
        "recovery",
        "no apt repository installation instructions",
    ] {
        assert!(
            source.contains(required),
            "apt repository design should cover {required}"
        );
    }

    Command::new(root.join("scripts/check-apt-repository-design.sh"))
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "apt repository design validation succeeded",
        ));
}

#[test]
fn opentelemetry_bridge_is_defined_without_default_network_telemetry() {
    let root = workspace_root();
    let design = root.join("docs/opentelemetry-bridge.md");
    assert!(design.exists(), "missing OpenTelemetry bridge design");

    let source = std::fs::read_to_string(&design).expect("design should be readable");
    for required in [
        "future OpenTelemetry bridge",
        "trace-to-metrics.sh",
        "trace-to-summary.sh",
        "file-based supervision contract",
        "no collectors by default",
        "no network telemetry by default",
        "deployment-owned bridge",
        "attribute mapping",
        "payload exclusion",
        "support commitment",
    ] {
        assert!(
            source.contains(required),
            "OpenTelemetry bridge design should cover {required}"
        );
    }

    Command::new(root.join("scripts/check-opentelemetry-bridge.sh"))
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "OpenTelemetry bridge validation succeeded",
        ));
}

#[test]
fn adoption_guide_demonstrates_real_agent_integration_patterns() {
    let root = workspace_root();
    let guide = root.join("docs/adoption/agent-runner.md");
    assert!(guide.exists(), "missing agent runner adoption guide");

    let source = std::fs::read_to_string(&guide).expect("guide should be readable");
    for required in [
        "bounded execution tool",
        "preflight",
        "dispatch",
        "supervision",
        "artifact collection",
        "retry decision",
        "failure_kind",
        "exit code",
        "checkpoint",
        "trace",
        "events",
        "batch-supervisor.py",
        "node-supervisor.mjs",
        "do not read prompt payloads from metadata",
    ] {
        assert!(
            source.contains(required),
            "agent runner adoption guide should cover {required}"
        );
    }

    Command::new(root.join("scripts/check-agent-adoption-guide.sh"))
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "agent adoption guide validation succeeded",
        ));
}

#[test]
fn release_preflight_runs_ecosystem_readiness_gate() {
    let root = workspace_root();
    let preflight = root.join("scripts/release-preflight.sh");
    assert!(preflight.exists(), "missing release preflight script");

    let source = std::fs::read_to_string(&preflight).expect("preflight should be readable");
    for required in [
        "scripts/check-ecosystem-readiness.sh",
        "scripts/check-agent-adoption-guide.sh",
        "scripts/check-opentelemetry-bridge.sh",
    ] {
        assert!(
            source.contains(required),
            "release preflight should include {required}"
        );
    }

    let readiness = std::fs::read_to_string(root.join("docs/release-readiness.md"))
        .expect("release readiness should be readable");
    assert!(readiness.contains("scripts/check-ecosystem-readiness.sh"));
    assert!(readiness.contains("scripts/check-agent-adoption-guide.sh"));
    assert!(readiness.contains("scripts/check-opentelemetry-bridge.sh"));
}

#[test]
fn release_notes_cover_current_ecosystem_contract() {
    let root = workspace_root();
    let version = env!("CARGO_PKG_VERSION");
    let tag = format!("v{version}");
    let notes_path = root.join(format!("docs/release-notes/{tag}.md"));
    assert!(notes_path.exists(), "missing current release notes");

    let source = std::fs::read_to_string(&notes_path).expect("release notes should be readable");
    assert!(source.contains(&format!("# llmff {tag}")));
    for required in [
        "Python subprocess supervisor",
        "batch supervisor",
        "Node.js streaming supervisor",
        "agent runner adoption guide",
        "OpenTelemetry bridge",
        "ecosystem readiness",
        "release preflight",
    ] {
        assert!(
            source.contains(required),
            "{tag} release notes should cover {required}"
        );
    }

    let preflight = std::fs::read_to_string(root.join("scripts/release-preflight.sh"))
        .expect("release preflight should be readable");
    assert!(preflight.contains("agent runner adoption guide"));
    assert!(preflight.contains("OpenTelemetry bridge"));
}

#[test]
fn package_manager_metadata_tracks_current_release_version() {
    let root = workspace_root();
    let workspace_version = env!("CARGO_PKG_VERSION");

    let checker = std::fs::read_to_string(root.join("scripts/check-package-manager-metadata.sh"))
        .expect("checker should be readable");
    let metadata_version = checker
        .lines()
        .find_map(|line| {
            line.strip_prefix("version=\"")
                .and_then(|rest| rest.strip_suffix('"'))
        })
        .expect("checker should define package-manager metadata version");
    let tag = format!("v{metadata_version}");
    assert!(
        parse_version_triplet(metadata_version) <= parse_version_triplet(workspace_version),
        "package metadata should not point past the workspace version"
    );
    assert!(checker.contains("tag=\"v${version}\""));

    for path in [
        "packaging/homebrew/llmff.rb",
        "packaging/scoop/llmff.json",
        "packaging/winget/Syndicalt.Llmff.installer.yaml",
        "packaging/aur/PKGBUILD",
        "packaging/aur/.SRCINFO",
    ] {
        let source = std::fs::read_to_string(root.join(path)).expect("file should be readable");
        assert!(
            source.contains(metadata_version),
            "{path} should reference metadata version {metadata_version}"
        );
        assert!(
            source.contains(&tag),
            "{path} should reference metadata tag {tag}"
        );
    }

    for path in [
        "packaging/winget/Syndicalt.Llmff.yaml",
        "packaging/winget/Syndicalt.Llmff.locale.en-US.yaml",
    ] {
        let source = std::fs::read_to_string(root.join(path)).expect("file should be readable");
        assert!(
            source.contains(metadata_version),
            "{path} should reference metadata version {metadata_version}"
        );
    }

    let roadmap = std::fs::read_to_string(root.join("docs/package-manager-roadmap.md")).unwrap();
    assert!(roadmap.contains(&format!("scripts/release-preflight.sh {tag}")));
    assert!(roadmap.contains(&format!("scripts/check-release-assets.sh {tag}")));

    Command::new(root.join("scripts/check-package-manager-metadata.sh"))
        .assert()
        .success()
        .stdout(predicates::str::contains(format!(
            "package-manager metadata validation succeeded for {tag}"
        )));
}

fn parse_version_triplet(version: &str) -> [u64; 3] {
    let mut parts = version.split('.');
    let major = parts
        .next()
        .and_then(|part| part.parse::<u64>().ok())
        .expect("major version should be numeric");
    let minor = parts
        .next()
        .and_then(|part| part.parse::<u64>().ok())
        .expect("minor version should be numeric");
    let patch = parts
        .next()
        .and_then(|part| part.parse::<u64>().ok())
        .expect("patch version should be numeric");
    assert!(
        parts.next().is_none(),
        "version should have exactly three components"
    );
    [major, minor, patch]
}
