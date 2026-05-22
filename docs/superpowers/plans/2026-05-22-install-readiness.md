# Install Readiness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `llmff` installable from GitHub for early testing and verify the installed binary through a repeatable smoke gate.

**Architecture:** Add one focused shell smoke script that installs into isolated Cargo directories and exercises the installed `llmff` binary. Update package metadata and README docs to present the install path separately from development checkout commands.

**Tech Stack:** Rust workspace, Cargo install, POSIX shell, existing example manifest, mock backend env vars, README and release-readiness docs.

---

## File Structure

- Create `scripts/smoke-install.sh`: isolated install and binary smoke gate.
- Create `docs/release-readiness.md`: advertise/release checklist.
- Modify `Cargo.toml`: correct repository metadata.
- Modify `README.md`: add installation section and direct `llmff` usage examples.
- Modify `docs/superpowers/plans/2026-05-22-install-readiness.md`: track execution.
- Create `docs/superpowers/specs/2026-05-22-install-readiness-design.md`: design record.

## Task 1: Smoke Gate Script

- [x] **Step 1: Write failing smoke gate invocation**

Run before creating the script:

```bash
scripts/smoke-install.sh --path .
```

Expected: FAIL with `No such file or directory`.

- [x] **Step 2: Create `scripts/smoke-install.sh`**

Create an executable shell script with this behavior:

```bash
#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
usage:
  scripts/smoke-install.sh --path <repo-path>
  scripts/smoke-install.sh --git <repo-url>
USAGE
}

if [ "$#" -ne 2 ]; then
  usage >&2
  exit 2
fi

mode="$1"
source="$2"
case "$mode" in
  --path|--git) ;;
  *)
    usage >&2
    exit 2
    ;;
esac

tmp="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp"
}
trap cleanup EXIT

export CARGO_HOME="$tmp/cargo-home"
export CARGO_TARGET_DIR="$tmp/cargo-target"
export PATH="$CARGO_HOME/bin:$PATH"
mkdir -p "$CARGO_HOME" "$CARGO_TARGET_DIR"

if [ "$mode" = "--path" ]; then
  repo="$(cd "$source" && pwd -P)"
  cargo install --path "$repo/crates/llmff-cli" --locked
  example="$repo/examples/json-repair.yaml"
else
  cargo install --git "$source" llmff --locked
  repo=""
  example="$tmp/json-repair.yaml"
  cat > "$example" <<'YAML'
version: 1
inputs:
  prompt:
    path: question.txt
graph:
  - id: load_prompt
    op: load
    input: prompt
  - id: draft
    op: infer
    from: load_prompt
    model: mock:bad
  - id: validate
    op: validate_json
    from: draft
    schema: '{"type":"object","required":["answer"]}'
  - id: repair
    op: repair
    from: validate
    when: invalid
    model: mock:good
outputs:
  final:
    from: repair
    path: answer.json
YAML
  cat > "$tmp/question.txt" <<'TXT'
Return an answer object.
TXT
fi

llmff --version | grep 'llmff '
llmff stages list | grep '^infer$'
llmff inspect "$example"

run_dir="$tmp/run"
mkdir -p "$run_dir"
if [ "$mode" = "--path" ]; then
  cp "$example" "$run_dir/pipeline.yaml"
  cp "$(dirname "$example")/question.txt" "$run_dir/question.txt"
else
  cp "$example" "$run_dir/pipeline.yaml"
  cp "$tmp/question.txt" "$run_dir/question.txt"
fi

(
  cd "$run_dir"
  LLMFF_MOCK_BAD_RESPONSE='{"wrong":true}' \
  LLMFF_MOCK_GOOD_RESPONSE='{"answer":"ok"}' \
    llmff run pipeline.yaml --trace trace.jsonl
  test "$(cat answer.json)" = '{"answer":"ok"}'
  llmff trace trace.jsonl | grep 'run cli-run succeeded'
)
```

- [x] **Step 3: Run GREEN and commit script/spec**

Run:

```bash
scripts/smoke-install.sh --path .
```

Expected: PASS.

Commit:

```bash
git add scripts/smoke-install.sh docs/superpowers/specs/2026-05-22-install-readiness-design.md docs/superpowers/plans/2026-05-22-install-readiness.md
git commit -m "test: add install smoke gate"
```

## Task 2: Install Docs and Metadata

- [x] **Step 1: Write failing metadata check**

Run:

```bash
grep -q 'https://github.com/syndicalt/llmff' Cargo.toml
```

Expected: FAIL because the workspace repository metadata uses the old owner.

- [x] **Step 2: Update metadata and docs**

Change `Cargo.toml`:

```toml
repository = "https://github.com/syndicalt/llmff"
```

Add README section near the top:

```markdown
## Install

Install from GitHub:

```bash
cargo install --git https://github.com/syndicalt/llmff llmff
```

For a local checkout:

```bash
cargo install --path crates/llmff-cli
```

Verify the installed binary:

```bash
llmff --version
llmff stages list
```
```

Convert user-facing examples from `cargo run -p llmff -- ...` to `llmff ...`, and add a short development note that checkout contributors can keep using `cargo run -p llmff -- ...`.

Create `docs/release-readiness.md` with the checklist from the design.

- [x] **Step 3: Run docs verification and commit**

Run:

```bash
grep -q 'https://github.com/syndicalt/llmff' Cargo.toml
grep -q 'cargo install --git https://github.com/syndicalt/llmff llmff' README.md
grep -q 'scripts/smoke-install.sh --path .' README.md docs/release-readiness.md
scripts/smoke-install.sh --path .
```

Commit:

```bash
git add Cargo.toml README.md docs/release-readiness.md docs/superpowers/plans/2026-05-22-install-readiness.md
git commit -m "docs: document GitHub install path"
```

## Task 3: Final Verification and Merge

- [x] **Step 1: Run full verification**

Run:

```bash
cargo fmt --all --check
cargo test --workspace
cargo run -p llmff -- inspect examples/json-repair.yaml
scripts/smoke-install.sh --path .
```

- [ ] **Step 2: Push, PR, merge, cleanup**

Push branch, create PR, merge when clean, fast-forward local `main`, remove worktree, and rerun:

```bash
cargo fmt --all --check
cargo test --workspace
scripts/smoke-install.sh --path .
```

## Self-Review

- Spec coverage: install command, local install, isolated smoke gate, metadata correction, docs, advertise checklist, and verification are covered.
- Placeholder scan: no placeholder implementation steps remain.
- Type consistency: paths and command names match existing workspace names: `crates/llmff-cli`, `examples/json-repair.yaml`, and `llmff`.
