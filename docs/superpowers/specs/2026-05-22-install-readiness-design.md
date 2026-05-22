# Install Readiness Design

## Purpose

Make `llmff` credible for early users to install from GitHub and run without cloning the repository. This slice does not publish a crate, cut a tag, or promise broad platform support. It creates a verified install path and a clear readiness checklist for public announcement.

## User-Facing Install Path

Primary install command:

```bash
cargo install --git https://github.com/syndicalt/llmff llmff
```

Development checkout install command:

```bash
cargo install --path crates/llmff-cli
```

After either install, users should be able to run:

```bash
llmff --version
llmff stages list
llmff inspect examples/json-repair.yaml
```

The README should lead with direct `llmff` commands for users and keep `cargo run -p llmff -- ...` as a development-checkout alternative.

## Smoke Gate

Add `scripts/smoke-install.sh`. It verifies the same install path that users will follow, but supports a local path for PR checks:

```bash
scripts/smoke-install.sh --path .
```

The script:

1. Creates a temporary home with isolated `CARGO_HOME` and `CARGO_TARGET_DIR`.
2. Installs `llmff` from either `--path <repo>` or `--git <url>`.
3. Prepends the temporary cargo bin directory to `PATH`.
4. Runs:
   - `llmff --version`
   - `llmff stages list`
   - `llmff inspect <repo>/examples/json-repair.yaml`
   - `llmff run <repo>/examples/json-repair.yaml --trace <tmp>/trace.jsonl` with mock responses.
   - `llmff trace <tmp>/trace.jsonl`
5. Asserts the run output and trace summary contain expected values.

The script must not rely on the repository's `target/` directory or a previously installed global `llmff`.

## Metadata Correction

The workspace package repository metadata should point to:

```toml
repository = "https://github.com/syndicalt/llmff"
```

The current remote is `git@github.com:syndicalt/llmff.git`, so this is required for consistent installation and package metadata.

## Advertise Readiness Checklist

Add a small checklist in `docs/release-readiness.md`:

- GitHub install smoke gate passes.
- README contains the install command and direct `llmff` examples.
- A versioned tag or release exists.
- Fresh install has been verified from that tag or release.
- Known limitations are documented.

After this slice, the project can be described as "installable from GitHub for early testing" if the smoke gate passes. It should not yet be described as broadly released until a tag/release gate is completed.

## Future Packaging Roadmap

Packaged installers are a future release-track capability, not part of this early GitHub install slice. The roadmap should include:

- Windows installer and signed `llmff.exe` archive.
- macOS installers or archives for Apple Silicon and Intel Macs, with signing and notarization before broad recommendation.
- Linux `.deb` packages for Ubuntu and Debian.
- Arch Linux package support, either through an official package recipe or an AUR-ready `PKGBUILD`.
- Plain compressed binary archives for each supported platform.
- CI-built artifacts, checksums, and platform smoke tests for every packaged release.

## Non-Goals

- No crates.io publish.
- No binary release artifacts in this early install-readiness slice.
- No Homebrew, apt, npm, or container packaging.
- No version bump or tag in this slice.
- No CI workflow addition unless the smoke script is ready to be wired later.

## Test Coverage

- Unit-level shell test equivalent: running `scripts/smoke-install.sh --path .` in a clean temp install root.
- Existing workspace tests continue to pass.
- README commands are backed by the smoke script and existing CLI tests.
