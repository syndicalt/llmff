# Ecosystem Readiness

`llmff` treats documented integration surfaces as public contracts. This page
maps each public integration path to the local validation gate or opt-in live
smoke gate that protects it.

## Integration Gates

| Integration path | Contract artifact | Validation gate | Gate type |
| --- | --- | --- | --- |
| Manifest contracts | `docs/schemas/pipeline-manifest-v1.schema.json`, `docs/manifest-reproducibility.md`, `docs/compatibility/core-contract-v1-matrix.json`, and `examples/*.yaml` | `python3 scripts/check-schema-contract.py` and `scripts/check-manifest-reproducibility.sh` | local |
| Trace and event streams | `docs/events.md` and `examples/supervision/fixtures/` | `cargo test -p llmff --test cli_run observability_export_scripts_summarize_trace_fixture` | local |
| OpenTelemetry bridge | `docs/opentelemetry-bridge.md` and local trace exporters | `scripts/check-opentelemetry-bridge.sh` | local |
| CLI JSON output | `docs/schemas/inspect-report-v1.schema.json` and CLI integration tests | `cargo test -p llmff --test cli_run inspect_json_reports_reproducible_execution_contract` | local |
| Plugin protocol | `docs/plugins/fixtures/protocol-v1/`, `docs/plugins/registry.v1.json`, `docs/plugins/promotion-policy.md`, and `docs/plugins/reviews/` | `scripts/check-plugin-fixtures.sh` | local |
| Provider onboarding | `docs/provider-troubleshooting.md`, `docs/provider-smoke-readiness.md`, `docs/providers/support-tiers.md`, `docs/providers/live-smoke-history.json`, `docs/providers/`, and `examples/providers/` | `scripts/check-provider-smoke-readiness.sh` and `.github/workflows/live-provider-smoke.yml` | local plus opt-in live smoke |
| Production workflow examples | `examples/real-world/` CI, queue worker, scheduled job, and failure triage examples | `scripts/check-real-world-workflows.sh` | local |
| Agent subprocess embedding | `docs/agent-workflows.md` and `examples/agent-workflows/` | `cargo test -p llmff --test example_catalog agent_workflow_docs_link_to_a_runnable_supervisor_example` | local |
| Agent runner adoption | `docs/adoption/agent-runner.md` and runnable agent workflow examples | `scripts/check-agent-adoption-guide.sh` | local |
| Package-manager metadata | `packaging/` and `docs/package-manager-roadmap.md` | `scripts/check-package-manager-metadata.sh` | local |
| Release assets | `.github/workflows/release-artifacts.yml` and `docs/distribution-trust.md` | `scripts/check-release-publication-wiring.sh` and `scripts/check-release-assets.sh <tag>` | local plus post-release |

## Promotion Policy

Registry promotion, package-manager publication, and live-provider
certification are support commitments. Metadata can be checked into this
repository before publication, but maintainers must explicitly accept ownership
for updates, user reports, rollback, and security fixes before a channel is
advertised as supported.

Local gates should run without secrets or network access. Live gates must be
explicitly opt-in and must document required secrets, runner assumptions, and
the provider or channel being certified.

Run the readiness index check with:

```bash
scripts/check-ecosystem-readiness.sh
```
