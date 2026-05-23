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
stages total=5 success=5 failed=0
timing run_wall_ms=54 total_stage_ms=48
stage load_prompt op=load status=success duration_ms=2
stage draft op=infer status=success duration_ms=30
stage cached op=cache status=success duration_ms=5
stage cache_miss op=cache status=success duration_ms=8
stage write_answer op=write status=success duration_ms=3
artifacts outputs=1 caches=2
artifact output stage=write_answer path=examples/out/answer.json
artifact cache stage=cached path=.llmff/cache/fixture.json hit=true
artifact cache stage=cache_miss path=.llmff/cache/miss.json hit=false
tokens prompt=12 completion=8 total=20
cache hits=1 misses=1 hit_rate=50.00%
backend_errors total=0 rate=0.00%
retries total=2 stages=1 max_attempts=3
failures total=0 backend=0 timeout=0
```

The metrics output is line-oriented text that can be scraped from a file or
converted later by a deployment-specific OpenTelemetry bridge:

```text
llmff_run_duration_ms 54
llmff_stage_duration_ms_sum 48
llmff_tokens_total 20
llmff_cache_hit_rate 0.5000
llmff_backend_error_rate 0.0000
llmff_retries_total 2
llmff_retry_stages_total 1
llmff_max_stage_attempts 3
llmff_failures_total 0
llmff_timeout_error_rate 0.0000
```
