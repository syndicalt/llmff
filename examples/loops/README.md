# Loop And Map Examples

These examples demonstrate the bounded iteration and collection stages added
for `llmff` v1.1. They are offline-first adoption fixtures: each manifest can
be inspected without provider credentials and can be run with the built-in mock
backend or local files.

The mock backend returns one configured response for every model call in a run,
so these examples prove loop control, tracing, inspect metadata, body-stage
operations, and output shape. To use them with a real provider, replace
`model: mock:good` with a registered provider model such as
`openai:gpt-4.1-mini` and run with
`--backend openai=https://api.openai.com/v1 --api-key-env openai=OPENAI_API_KEY`.

## Self-Refining Answer

Use this when a draft should be validated each iteration and accepted as soon
as a typed predicate passes.

```bash
llmff inspect examples/loops/self-refining-answer-loop.yaml
LLMFF_MOCK_GOOD_RESPONSE='{"answer":"Use llmff for bounded, inspectable LLM pipelines.","confidence":0.93}' \
llmff run examples/loops/self-refining-answer-loop.yaml \
  --trace /tmp/llmff-self-refining-answer.trace.jsonl
```

Operations shown: `loop`, `validate_json`, `predicate`, and `extract`.

## ReAct-Style Tool Loop

Use this as the v1.1-safe shape for a ReAct-style controller. The model emits a
typed tool request, `validate_json` checks it, `tool` runs a deterministic local
subprocess fixture, and `accumulate` carries observations into the next
iteration. The loop stops when the model request says the task is done.

```bash
llmff inspect examples/loops/react-style-tool-use-loop.yaml
LLMFF_MOCK_GOOD_RESPONSE='{"tool":"direct","args":{},"done":true,"final_answer":"Use a bounded loop and inspect the trace."}' \
llmff run examples/loops/react-style-tool-use-loop.yaml \
  --trace /tmp/llmff-react-style-tool-use.trace.jsonl
```

Operations shown: `loop`, `validate_json`, `predicate`, `tool`, and
`accumulate`.

## Best-of-N Sampling And Selection

Use this when you want a fixed number of candidate iterations, retained
iteration summaries, and an in-pipeline winner selection step. The loop always
runs the configured count, scores each candidate, retains the score stage for
every iteration, and a downstream `select` stage chooses the highest score.

```bash
llmff inspect examples/loops/best-of-n-sampling+selection-loop.yaml
LLMFF_MOCK_GOOD_RESPONSE='{"candidate":"Candidate answer from a bounded sample.","score":8}' \
llmff run examples/loops/best-of-n-sampling+selection-loop.yaml \
  --trace /tmp/llmff-best-of-n.trace.jsonl
```

Operations shown: `loop` with `break_on: never`, `retain_iterations`, `score`,
and `select`.

## Iterative Research And Fact Check

Use this when retrieval, synthesis, and validation should repeat until claims
are supported or the iteration bound is reached. The example carries a compact
claim history between iterations instead of giving the loop implicit memory.

```bash
llmff inspect examples/loops/iterative-research-fact-check-loop.yaml
LLMFF_MOCK_GOOD_RESPONSE='{"supported":true,"claims":["Rust and Python are available in the local context."],"sources":["retrieval/rust.txt","retrieval/python.txt"]}' \
llmff run examples/loops/iterative-research-fact-check-loop.yaml \
  --trace /tmp/llmff-research-loop.trace.jsonl
```

Operations shown: `loop`, `retrieve`, `validate_json`, `predicate`, `extract`,
and `accumulate`.

## Map Batch Items

Use this when the manifest should apply a bounded body graph to items in a JSON
array. This is distinct from CLI batch mode: `op: map` is a stage inside one
pipeline run, while `--batch-input` runs the whole manifest once per input
line.

```bash
llmff inspect examples/loops/map-batch-items.yaml
llmff run examples/loops/map-batch-items.yaml \
  --trace /tmp/llmff-map-batch-items.trace.jsonl
```

Operations shown: `map`, `items_from`, `max_items`, and the reserved body input
`item`.

## Tool Request And Result Contracts

Keep tool loops typed at both edges. Validate the model-produced tool request
before invoking `tool`, then validate the tool result before it is accumulated
or fed back into another model call.

The ReAct example uses this request shape:

```json
{
  "tool": "direct",
  "args": {},
  "done": true,
  "final_answer": "Use a bounded loop and inspect the trace."
}
```

The local fixture returns:

```json
{
  "ok": true,
  "result": {
    "final_answer": "Use a bounded loop and inspect the trace.",
    "tool": "direct"
  }
}
```

## Inspect Bounds

Use JSON inspect output when a supervisor needs to budget loop or map work
before execution:

```bash
llmff inspect examples/loops/self-refining-answer-loop.yaml --format json
llmff inspect examples/loops/map-batch-items.yaml --format json
```

Loop stages report `max_iterations`, `body_stage_count`,
`max_expanded_stage_count`, `break_on`, retention settings, and final stage
metadata. Map stages report their item source, body stage count, item cap, and
maximum expanded stage count.

## Clean Up Generated Outputs

```bash
rm -f examples/loops/*.output.json examples/loops/*.output.txt /tmp/llmff-*.trace.jsonl
```
