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
