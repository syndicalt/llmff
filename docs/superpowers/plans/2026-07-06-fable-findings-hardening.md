# Fable Findings Hardening

**Goal:** Resolve the nine defects identified in the 2026-07-06 codebase review
(typed error classification, exhaustive stage-op wiring, parse-time stage spec
validation, engine decomposition, check-script and test-harness economy, gate
de-ceremony, doc dedup, article snippet validity) with zero observable contract
change.

**Architecture:** Internal refactors only. The flat `StageSpec` remains the
serde wire format; typed representations are constructed after deserialization.
`LlmffError` gains dedicated variants whose `Display` output is byte-identical
to today's message strings, so exit codes, failure kinds, trace/event JSON,
schemas, and fixtures are unchanged.

**Tech Stack:** Existing (Rust workspace, thiserror core / anyhow CLI, bash
check gates, Python schema checker).

## Scope

Ship in this slice:

- Typed failure classification in `llmff-core`; `exit_codes.rs` and
  `engine/trace_failure.rs` match variants, never message text. Shared single
  classifier; characterization tests capture the current mapping table first.
- Internal `StageOp` enum used exhaustively by the metadata catalog,
  deterministic dispatch, engine dispatch, and all three `graph.rs` validator
  call sites.
- Typed per-op spec structs constructed at graph-build time; deterministic
  stage impls consume them. Validation error messages unchanged.
- `engine.rs` production-code extraction into cohesive `engine/` submodules
  (pure code motion).
- `scripts/lib/checks.sh` shared helpers sourced by every `check-*.sh`.
- `check-release-runbook.sh` structural evidence assertions replacing pinned
  commit SHA / CI run ID values.
- `crates/llmff-cli/tests/` decomposition of `cli_run.rs` with shared harness
  helpers; all existing assertions preserved.
- Readiness-doc dedup with one canonical home per statement and
  cross-references; gates updated in lockstep without weakening intent.
- Every YAML snippet in the v1.1/v1.2 launch articles verified by
  `llmff inspect`.

Do not ship in this slice:

- Any change to documented CLI output, exit codes, failure kinds, schemas,
  fixtures, plugin protocol, or manifest surface.
- New stages, backends, or features of any kind.
- Deprecation or removal of any public doc statement a gate depends on,
  except the point-in-time value pins named above.

## Compatibility note

Adding variants to the public `LlmffError` enum is a breaking change for
library consumers who match it exhaustively. Accepted by maintainer decision
2026-07-06 (young library ecosystem; CLI/schema/event contracts — the
supported surfaces — are unaffected).

## Verification

- Characterization tests for the full (error → exit code, failure kind,
  failure message) table pass before and after.
- `cargo fmt --check`, `clippy -D warnings`,
  `cargo test --workspace --locked --no-fail-fast`,
  `scripts/check-schema-contract.py`, plugin/governance/ecosystem/real-world
  gates, `smoke-events-streaming.sh` — all green at baseline (337 tests) and
  after every task.

## Tasks

- [x] A1 typed error classification
- [x] A2 exhaustive `StageOp` enum
- [x] A3 typed per-op stage specs (incl. retrieve/rerank follow-up)
- [x] A4 engine submodule extraction
- [x] B1 shared check-script library
- [x] B3 structural runbook gate
- [x] B2 cli_run.rs decomposition
- [x] C1 readiness-doc dedup
- [x] C2 article snippet verification
- [x] Final integration gate + adversarial review

## Completion decisions (2026-07-06)

- Graph-time validation only ever covered 7 of 19 ops; the other 12 are
  enforced by `Engine::validate_stage` on the execution path (exit 20). Typed
  specs for those ops parse at the execution site so no failure moves from
  exit 20 to exit 10. `parse` returns a message `String`; each call site wraps
  it in the contextually correct error variant.
- Release-evidence commit SHAs are verified against git ground truth
  (`require_release_commit_matches_tag`: recorded SHA must equal
  `git rev-list -n 1 <tag>` when the tag is resolvable; shape check otherwise).
  CI run ids stay label-anchored shape checks: they have no local ground
  truth, and pinning a literal copied from the file being checked verifies
  nothing.
- Accepted wording unification: `predicate`/`accumulate` invalid-mode messages
  on the direct-library execution path (unreachable via CLI, which validates
  at graph build) now use the graph validator's tested wording. Exit codes and
  failure kinds unaffected.
- `loop`/`map` field validation stays graph-structural by design; `tool`'s
  HTTP retry loop reads method/url/headers from the raw spec after
  `ToolSpec::parse` guarantees presence (documented in `stage/specs.rs`).
