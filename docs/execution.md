# Execution Controls

`llmff run` executes stages sequentially by default. Use `--parallel` to run
ready independent stages concurrently.

## Limits

- `--max-concurrency N` caps concurrently running stages in the parallel
  scheduler. `N` must be greater than zero.
- `--timeout-ms N` sets a default per-stage timeout. A manifest stage can
  override it with `timeout_ms`.

## Retries

Model stages (`infer`, `repair`) and HTTP tool stages can retry failed attempts.
CLI defaults apply to all eligible stages:

```bash
llmff run --retry-attempts 3 --retry-backoff-ms 250 pipeline.yaml
```

Manifest stages can override the retry policy:

```yaml
retry:
  attempts: 3
  backoff_ms: 250
```

`attempts` is the total number of tries, including the first attempt.
HTTP tool retries are limited to transport errors and server-side 5xx
responses. Client-side 4xx responses are treated as permanent failures.

## Loop Stages

`op: loop` executes an embedded body graph sequentially. Every loop must declare
`max_iterations`, `break_on`, and `body`. The executor stops when the break
condition is satisfied or when `max_iterations` is reached.

The body is a DAG. Body stages may reference `input`, earlier body stages, or
named values supplied through `carry`. Body stages must not route back to
earlier stages; repetition belongs to the loop controller.

The loop stage output is a JSON value:

```json
{
  "final": {},
  "metadata": {
    "iterations_run": 1,
    "stop_reason": "break_condition",
    "final_stage": "draft"
  }
}
```

Trace and event records for body stages include `loop_id`, `loop_iteration`,
and `loop_stage_id`.

Predicate stages can produce an explicit JSON break signal for loop bodies:

```yaml
- id: ready
  op: predicate
  from: scored
  field: score
  mode: gte
  value: 7

break_on:
  type: field_true
  stage: ready
  field: passed
```

## Map Stages

`op: map` applies an embedded body graph to items from a JSON array inside one
pipeline run. It is bounded by `max_items`; if the input array is longer, only
the first `max_items` values are processed. This is separate from CLI batch
mode, which runs the whole manifest once per input line.

Map execution is sequential by default. Set `parallel: true` with required
`max_concurrency` to process items concurrently while preserving deterministic
output order by item index.

```yaml
- id: map_names
  op: map
  from: load_payload
  items_from: items
  max_items: 3
  parallel: true
  max_concurrency: 2
  body:
    - id: name
      op: extract
      from: item
      field: name
```

Each map body receives the reserved `item` value for the current array entry.
The map stage output is a JSON value:

```json
{
  "items": [],
  "metadata": {
    "items_run": 3,
    "items_total": 5,
    "stop_reason": "max_items",
    "parallel": true
  }
}
```

Use `llmff inspect --format json` to read `items_from`, `max_items`,
`parallel`, `max_concurrency`, `body_stage_count`, and
`max_expanded_stage_count` before dispatch. Trace and event records for map
body stages include `map_id`, `map_index`, and `map_stage_id`.

## Cache Policy

Cache stages default to `cache_policy: read`, which reuses an existing matching
cache entry. `cache_policy: refresh` recomputes and replaces the matching entry.
`cache_policy: bypass` returns the parent value without reading or writing the
cache.

## Checkpoints And Resume

`--checkpoint path` writes completed stage statuses after each stage finishes.
`--resume path` starts from a previous checkpoint and skips stages already in the
checkpoint. Checkpoints are bound to a hash of the current manifest so stale
checkpoints cannot be silently reused with a changed graph. Checkpoints store
stage values, so treat them as job artifacts.

If a checkpoint cannot be reused because its manifest hash differs from the
current manifest, `llmff` exits with code `10` and reports the checkpoint path,
the checkpoint's saved manifest hash, the current manifest hash, and an
`inspect --format json` hint. Supervisors should treat this as a static
preflight failure: do not retry the same checkpoint against the changed
manifest.

## Batch Input Mode

`--batch-input path --batch-output-dir dir` runs the same manifest once for each
line in the batch input file. Batch mode requires exactly one manifest input and
file outputs. Each line is written to an isolated per-item input file, and each
manifest output is rewritten under `dir/items/<index>/` so items cannot
overwrite each other.

```bash
llmff run --batch-input prompts.txt --batch-output-dir .llmff/batch pipeline.yaml
```

The runner writes `dir/batch-report.jsonl` with one status object per item. If
any item fails, the command keeps the report on disk and exits non-zero after
processing the batch.

## Replay

`--replay-trace path` validates whether a trace has completed stages. It can be
combined with `--resume checkpoint.json` when the checkpoint was produced by the
same manifest. Full trace-only replay is intentionally rejected because traces
omit prompts, tool bodies, cached values, and secrets.
