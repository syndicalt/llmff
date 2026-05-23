# Local Dashboard Example

This example keeps observability local. It reads a trace JSONL file and prints a
small terminal dashboard without sending events to a network service.

```bash
trace=/tmp/llmff-trace.jsonl
LLMFF_MOCK_GOOD_RESPONSE='{"answer":"ok"}' \
llmff run examples/json-repair.yaml --trace "$trace"

scripts/trace-to-summary.sh "$trace"
scripts/trace-to-metrics.sh "$trace"
```

The summary output is intended for humans:

```text
run fixture-run success
stages total=4 success=4 failed=0
timing total_stage_ms=45
tokens prompt=12 completion=8 total=20
cache hits=1 misses=1 hit_rate=50.00%
backend_errors total=0 rate=0.00%
```

The metrics output is line-oriented text that can be scraped from a file or
converted later by a deployment-specific OpenTelemetry bridge:

```text
llmff_stage_duration_ms_sum 45
llmff_tokens_total 20
llmff_cache_hit_rate 0.5000
llmff_backend_error_rate 0.0000
```
