# Loop Examples

These examples demonstrate the bounded `op: loop` stage added in `llmff`
v1.1. They are offline-first adoption fixtures: each manifest can be inspected
without provider credentials and can be run with the built-in mock backend.

The mock backend returns one configured response for every model call in a run,
so these examples prove loop control, tracing, inspect metadata, and output
shape. To use them with a real provider, replace `model: mock:good` with a
registered provider model such as `openai:gpt-4.1-mini` and run with
`--backend openai=https://api.openai.com/v1 --api-key-env openai=OPENAI_API_KEY`.

## Self-Refining Answer

Use this when a draft should be validated each iteration and accepted as soon
as it satisfies a schema.

```bash
llmff inspect examples/loops/self-refining-answer-loop.yaml
LLMFF_MOCK_GOOD_RESPONSE='{"answer":"Use llmff for bounded, inspectable LLM pipelines.","confidence":0.93}' \
llmff run examples/loops/self-refining-answer-loop.yaml \
  --trace /tmp/llmff-self-refining-answer.trace.jsonl
```

Loop feature shown: `break_on: stage_success`.

## ReAct-Style Tool Loop

Use this as the v1.1-safe shape for a ReAct-style controller. The example keeps
tool execution simulated by returning a completed JSON decision from the model
so it runs offline; a real version can insert an HTTP, command, or plugin
`tool` stage before the next decision step.

```bash
llmff inspect examples/loops/react-style-tool-use-loop.yaml
LLMFF_MOCK_GOOD_RESPONSE='{"thought":"The answer can be produced directly.","action":"answer","task_complete":true,"final_answer":"Use a bounded loop and inspect the trace."}' \
llmff run examples/loops/react-style-tool-use-loop.yaml \
  --trace /tmp/llmff-react-style-tool-use.trace.jsonl
```

Loop feature shown: `break_on: field_true`.

## Best-of-N Sampling Skeleton

Use this when you want a fixed number of candidate iterations. v1.1 loops are
sequential and do not include a built-in selector stage, so this example writes
the last scored candidate. Use a downstream plugin or supervisor to select
across retained traces or output artifacts when you need true best-of-N
selection.

```bash
llmff inspect examples/loops/best-of-n-sampling+selection-loop.yaml
LLMFF_MOCK_GOOD_RESPONSE='{"candidate":"Candidate answer from a bounded sample.","score":8}' \
llmff run examples/loops/best-of-n-sampling+selection-loop.yaml \
  --trace /tmp/llmff-best-of-n.trace.jsonl
```

Loop feature shown: `break_on: never`.

## Iterative Research And Fact Check

Use this when retrieval, synthesis, and validation should repeat until claims
are supported or the iteration bound is reached.

```bash
llmff inspect examples/loops/iterative-research-fact-check-loop.yaml
LLMFF_MOCK_GOOD_RESPONSE='{"supported":true,"claims":["Rust and Python are available in the local context."],"sources":["retrieval/rust.txt","retrieval/python.txt"]}' \
llmff run examples/loops/iterative-research-fact-check-loop.yaml \
  --trace /tmp/llmff-research-loop.trace.jsonl
```

Loop feature shown: `break_on: field_true` after retrieval and JSON
validation.

## Inspect Loop Bounds

Use JSON inspect output when a supervisor needs to budget loop work before
execution:

```bash
llmff inspect examples/loops/self-refining-answer-loop.yaml --format json
```

The loop stage reports `max_iterations`, `body_stage_count`,
`max_expanded_stage_count`, `break_on`, and `final` metadata.

## Clean Up Generated Outputs

```bash
rm -f examples/loops/*.output.json examples/loops/*.output.txt /tmp/llmff-*.trace.jsonl
```
