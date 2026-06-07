# WisePick -> llmff -> Eventloom Flow

This is an external composition harness. It validates a runtime loop around
`llmff` without adding WisePick or Eventloom as `llmff` runtime dependencies.

The boundary is:

```text
POST /v1/decide
  -> llmff run
  -> Eventloom-compatible JSONL
  -> POST /v1/feedback
```

`llmff` remains the bounded execution layer. The harness owns the WisePick HTTP
calls, capability-to-manifest mapping, Eventloom-compatible JSONL output, and
feedback submission.

## Dry Run

Use dry-run mode to verify the local artifact contract without network access
or a built `llmff` binary:

```bash
python3 examples/wisepick-eventloom-flow/run.py \
  --dry-run \
  --intent "Clean and return this record as JSON" \
  --out-dir /tmp/llmff-wisepick-flow
```

The dry run writes:

```text
/tmp/llmff-wisepick-flow/eventloom-compatible.jsonl
```

It includes planned `routing.decide.requested`, `routing.decided`,
`llmff.execution.planned`, and `routing.feedback.planned` records.

## Eventloom Import Mode

Add `--eventloom-log` when you want the harness to append each source journal
record into a sealed Eventloom log as it runs:

```bash
EVENTLOOM_BIN=eventloom \
python3 examples/wisepick-eventloom-flow/run.py \
  --dry-run \
  --intent "Clean and return this record as JSON" \
  --out-dir /tmp/llmff-wisepick-flow \
  --eventloom-log .eventloom/wisepick-llmff.jsonl
```

The harness still writes `eventloom-compatible.jsonl`, but the configured
Eventloom log is the canonical hash-chained provenance log. Verify and bundle
that log with Eventloom:

```bash
eventloom verify .eventloom/wisepick-llmff.jsonl
eventloom artifacts .eventloom/wisepick-llmff.jsonl \
  --out .eventloom/wisepick-llmff-artifacts \
  --title "WisePick llmff Validation"
eventloom artifacts verify .eventloom/wisepick-llmff-artifacts/manifest.json
```

Commit the sealed log and artifact bundle for provenance:

```bash
git add .eventloom/wisepick-llmff.jsonl .eventloom/wisepick-llmff-artifacts
git commit -m "test: record wisepick llmff validation flow"
```

## Hosted Endpoint Validation

Use mock-WisePick mode when you want to run the `llmff` subprocess locally
without calling the hosted endpoint:

```bash
cargo build -p llmff

LLMFF_BIN=target/debug/llmff \
python3 examples/wisepick-eventloom-flow/run.py \
  --mock-wisepick \
  --intent "Clean and return this record as JSON" \
  --out-dir /tmp/llmff-wisepick-flow
```

This mode writes the same Eventloom-compatible JSONL journal and native `llmff`
artifacts, but it records WisePick feedback as planned instead of sending
`POST /v1/feedback`.

Run the full flow with WisePick's hosted endpoint:

```bash
cargo build -p llmff

WISEPICK_API_URL=https://api.wishweaver.top \
LLMFF_BIN=target/debug/llmff \
python3 examples/wisepick-eventloom-flow/run.py \
  --intent "Clean and return this record as JSON" \
  --out-dir /tmp/llmff-wisepick-flow
```

The harness:

1. Calls WisePick `POST /v1/decide` with the intent.
2. Maps the returned `capability_id` to a local `llmff` example manifest.
3. Runs `llmff run` as a subprocess with file-backed events and trace output.
4. Writes an Eventloom-compatible JSONL journal for the routing and execution
   lifecycle.
5. Calls WisePick `POST /v1/feedback` with success, latency, token, and quality
   fields.

## Artifacts

The output directory contains:

- `eventloom-compatible.jsonl`: external journal records for routing,
  execution, and feedback.
- `llmff-events.jsonl`: native `llmff run --events` lifecycle events.
- `llmff-trace.jsonl`: native `llmff` trace.
- `llmff-checkpoint.json`: checkpoint artifact for the example run.
- `pipeline/`: copied offline `json-repair.yaml` fixture and inputs.

The journal is intentionally small and append-only so an Eventloom-side example
can ingest or translate it without changing `llmff` core.

## Boundary Notes

- WisePick owns routing decisions and feedback learning.
- `llmff` owns typed pipeline execution and local run artifacts.
- Eventloom owns replayable event-log semantics if a caller chooses to ingest
  the JSONL journal.
- This example does not add a WisePick stage, Eventloom dependency, serving
  layer, scheduler, memory layer, or agent framework to `llmff`.
