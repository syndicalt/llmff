# Manifest Reproducibility

`llmff inspect --format json` is the current reproducibility surface. It should
let operators audit a run before execution without reading prompts, tool
bodies, backend payloads, or generated outputs.

## Inspect Contract

The inspect report must expose:

- manifest hash;
- schema version and inspect report format version;
- resolved inputs, including file paths, stdin, and batch input when present;
- resolved outputs, including generated artifacts and stdout ownership;
- stage order after dependency resolution;
- backend aliases and provider registration metadata;
- model ids as written in stages and provider model ids after alias
  resolution;
- plugin dependencies and plugin protocol version;
- cache policy for cache stages;
- checkpoint/resume policy, including requested checkpoint and resume paths;
- requested execution controls such as scheduler, timeout, retry, and streaming
  options;
- known capability constraints for each stage.

These fields are metadata. They should remain safe to write to CI logs and
agent supervisor state.

## Lockfile Decision

The manifest lockfile remains parked. A lockfile is useful only if it
materially improves portability across machines and provider configurations
beyond the current inspect report. Today, `llmff` does not resolve mutable
remote model versions, package dependencies, or provider-side deployment hashes
that could be faithfully locked by the CLI.

Revisit manifest-lock support only when at least one of these is true:

- provider APIs expose stable deployment revisions that `llmff` can resolve and
  verify later;
- plugin registries introduce versioned remote dependencies;
- cache, retrieval, or model artifacts need a portable replay bundle;
- downstream supervisors need a signed or reviewable execution bill of
  materials that cannot be derived from `inspect --format json`.

Until then, adding a lockfile would mostly duplicate inspect output and create
false confidence. Use the inspect report, schema fixtures, and compatibility
checks as the reproducibility contract.

## Local Gate

Run:

```bash
scripts/check-manifest-reproducibility.sh
```

The gate validates that the guide, inspect schema, inspect golden fixture, and
roadmap stay aligned.
