# Real-World Workflow Examples

These examples show `llmff` as a supervised job runner from common production
automation contexts. They run offline by default with deterministic mock model
responses and write generated manifests, outputs, traces, events, checkpoints,
and result summaries into a caller-provided work directory.

Point examples at a development binary with `LLMFF_BIN`:

```bash
LLMFF_BIN=target/debug/llmff python3 examples/real-world/ci-job.py
```

## CI Job

`ci-job.py` performs an inspect preflight, runs a bounded issue-triage workflow
with `--run-dir`, checks the declared output artifact, prints the process exit
code and `result.json` status, and exits non-zero when the llmff run fails.

## Queue Worker

`queue-worker.py` writes two queue messages to JSONL batch input, runs batch
classification with isolated per-item artifacts, and acknowledges only items
that have succeeded in the batch report and have an output artifact.

## Scheduled Job

`scheduled-job.py` runs a meeting-notes extraction job as a cron-style task. On
success it records a small scheduler state file with the manifest hash, run
directory, and output path; on failure it preserves the llmff exit code.

## Failure Triage

`failure-triage.py` intentionally sends invalid mock output through a schema
validation workflow. The wrapper treats the failing llmff process as expected
input for the supervisor, reads `result.json` and `events.jsonl`, prints the
exit code, `failure_kind`, and retry decision, then exits zero only when the
expected failure class is observed.

Validate the complete set:

```bash
scripts/check-real-world-workflows.sh
```
