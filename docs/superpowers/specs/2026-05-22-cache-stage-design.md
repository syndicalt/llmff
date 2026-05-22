# Cache Stage Design

## Purpose

Add a deterministic `cache` stage so `llmff` pipelines can persist and reuse successful intermediate values across runs. This makes repeated local pipeline execution faster and more reproducible without adding a backend-specific serving layer or hidden environment requirements.

## Stage Shape

Manifest form:

```yaml
graph:
  - id: cached_prompt
    op: cache
    from: render_prompt
    path: .llmff/cache
    key: prompt-v1
```

Fields:

- `from`: required parent stage id.
- `path`: optional cache directory. Defaults to `.llmff/cache`.
- `key`: optional namespace string. When omitted, the stage id is used.

The stage accepts only successful parent values. Invalid and skipped parents fail in the same explicit style as `write`.

## Cache Key and Stored Data

The cache key is stable across runs for the same stage configuration and cache namespace. When `key` is present, the key is explicit and does not include the parent value; this lets a manifest author intentionally reuse the cached value behind a semantic name such as `prompt-v1`. When `key` is omitted, the parent value participates in the digest so the default behavior is content-addressed.

For explicit keys, the digest preimage contains:

- cache format version
- stage id
- cache key namespace

For implicit keys, the digest preimage contains:

- cache format version
- stage id
- stage id as the namespace
- parent value

The first implementation intentionally does not include wall-clock time, environment variables, output paths, or trace paths in the key. The cache file name is the lowercase hexadecimal SHA-256 digest with `.json` extension.

Each cache file stores:

```json
{
  "version": 1,
  "value": {
    "Text": "..."
  }
}
```

`Value` already derives `Serialize` and `Deserialize`, so cached values preserve text, messages, and JSON without ad hoc string parsing.

## Execution Semantics

On a miss:

1. Resolve the cache directory relative to the manifest directory unless absolute.
2. Create the directory if it does not exist.
3. Serialize the parent `Value` to the cache record JSON.
4. Write through a temporary file in the same directory and rename it into place.
5. Return the parent value unchanged.

On a hit:

1. Read the matching cache file.
2. Validate the cache record version.
3. Deserialize the stored value.
4. Return the cached value.

Corrupt or incompatible cache files fail the run with a `StageExecution` error naming the cache stage. The stage does not silently ignore bad cache data because silent misses hide reproducibility problems.

## Trace Metadata

`stage_finished` trace events gain optional cache metadata:

- `cache_hit`: `true` for hit, `false` for miss.
- `cache_path`: the cache file path configured by the stage.

Trace metadata must not include the cache key preimage or cached value.

## CLI and Docs

`llmff stages list` includes `cache`.

README documents the manifest form, default path, hit/miss behavior, and states that cache env vars are not required. The limitations list removes `cache stages`.

Inline graph support is not part of this slice because `path` and `key` syntax should be designed with broader parameter support rather than special-cased only for cache.

## Non-Goals

- No automatic caching of `infer` or other stages.
- No TTL, eviction, locking, compression, remote cache, or cache invalidation commands.
- No secrets handling beyond the existing guidance that manifests and traces should not store secrets.
- No caching of failed, invalid, or skipped statuses.
- No environment flags.

## Test Coverage

- Manifest parsing for `key`.
- Engine validation rejecting cache without `from`.
- Engine cache miss creates a cache file and returns the parent value.
- Engine cache hit returns the cached value even when the parent value changes.
- Trace metadata records cache hit and miss without cached contents.
- CLI stage listing includes `cache`.
- CLI run executes a cache stage end to end.
