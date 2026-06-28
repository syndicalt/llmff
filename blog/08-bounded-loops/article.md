# Bounded Loops Without Becoming An Agent Framework

v1.1 adds loops, but it does not add autonomous agents. A loop is a bounded stage with an embedded body graph, explicit break condition, and traceable per-iteration execution.

[IMAGE PLACEHOLDER: Unrolled loop diagram showing one outer loop stage expanded into three body DAG copies. Each copy should include draft -> validate -> score/check, with trace labels `(loop_id, iteration, stage_id)` and a break gate that can stop before the static upper bound.]

A loop is where LLM workflow code usually starts to lose its shape. I've watched the progression enough times to describe it from memory: one retry becomes a while loop, the while loop grows a memory list, the memory list becomes a scratchpad, the scratchpad starts carrying policy — and soon the code that was supposed to run a bounded task is quietly deciding what the task is.

`llmff` takes a narrower position. Loops are useful; unbounded autonomy is not an execution primitive. A loop without a bound is a policy decision, and `llmff` requires the bound. That's why `op: loop` demands a maximum iteration count, a break condition, and a body graph — the loop repeats declared work, and it never gets to become an agent language.

## The loop is one stage in the outer graph

The smallest shape looks like this:

```yaml
version: 1
inputs:
  prompt:
    path: ../question.txt
graph:
  - id: load_prompt
    op: load
    input: prompt

  - id: refine
    op: loop
    from: load_prompt
    max_iterations: 3
    break_on:
      type: stage_success
      stage: check
    final:
      from: draft
      require_status: success
    body:
      - id: draft
        op: infer
        from: input
        model: mock:json
        response_format: json
        temperature: 0

      - id: check
        op: validate_json
        from: draft
        schema: '{"type":"object","required":["answer"],"properties":{"answer":{"type":"string"}}}'
outputs:
  final:
    from: refine
    path: ./self-refine-loop.answer.json
```

The outer graph sees `refine` as one stage; inside it is a body DAG. The body can refer to `input` — the loop input for the current iteration — and to earlier body stages like `draft`, using normal operations: `infer`, `validate_json`, `predicate`, `extract`, `tool`, `accumulate`, `score`, and the rest.

What the body cannot do is route back to itself. Repetition belongs to the loop controller, and that division is what keeps the whole structure inspectable: the body stays a DAG, and the repetition is a bounded unrolling of that DAG. Mathematically:

```text
Loop(G_body, N) -> G_body^1, G_body^2, ... G_body^N
```

`break_on` may stop execution earlier than `N`. Nothing can make it run longer.

## Bounds are part of the contract

`max_iterations` is required because supervisors need an upper bound before execution, not after. For a loop with five body stages and `max_iterations: 4`, the static expansion bound is:

```text
max_expanded_stage_count = body_stage_count * max_iterations
                         = 5 * 4
                         = 20
```

That's an upper bound, not a prediction — a break condition may stop at iteration one, and guards may change which stages run. But the supervisor knows the maximum size of the work before allowing it, and the same logic extends to cost ceilings, event volume, trace size, and wall-clock budget. Static bounds don't solve all of those problems; what they provide is a real control surface instead of a promise hidden inside Python code.

Inspect is that surface:

```bash
llmff inspect examples/loops/self-refining-answer-loop.yaml --format json
```

```json
{
  "id": "refine_loop",
  "op": "loop",
  "loop": {
    "max_iterations": 5,
    "body_stage_count": 5,
    "max_expanded_stage_count": 25,
    "break_on": {
      "type": "field_true",
      "stage": "quality_ready",
      "field": "passed"
    },
    "on_iteration_error": "fail",
    "retain_iterations": "none"
  }
}
```

A supervisor can reject that loop before a single model call happens.

## Break conditions are explicit

A loop must say when it stops. For fixed sampling, the honest answer is "it doesn't stop early," and the manifest can say exactly that with `type: never`:

```yaml
- id: best_of_n_loop
  op: loop
  from: build_sampling_prompt
  max_iterations: 5
  break_on:
    type: never
  retain_iterations:
    mode: all
    stages: [score_candidate]
    include_values: true
  final:
    from: score_candidate
    require_status: success
  body:
    - id: generate
      op: infer
      from: input
      model: mock:good
      response_format: json
      temperature: 0.9

    - id: validate_candidate
      op: validate_json
      from: generate
      schema: '{"type":"object","required":["candidate","score"],"properties":{"candidate":{"type":"string"},"score":{"type":"number","minimum":0,"maximum":10}}}'

    - id: score_candidate
      op: score
      from: validate_candidate
      score_field: score
      min_score: 0
      max_score: 10
```

This is best-of-N, not open-ended exploration. The loop runs the configured count, retains the selected iteration data, and a downstream stage picks the winner:

```yaml
- id: best_candidate
  op: select
  from: best_of_n_loop
  json_path: iterations
  mode: highest_score
  score_field: stages.score_candidate.value.score
```

For refinement, the break condition references a body stage (`type: stage_success, stage: check`); for ReAct-style tool use, it references a predicate stage (`type: field_true, stage: task_done, field: passed`). The exact predicate type matters less than the principle: stopping behavior is declared in the manifest and visible to inspect, rather than being an emergent property of host code.

## Carry is explicit state, not memory

Some loops genuinely need state between iterations — a tool-use loop needs observation history, a research loop may need a short claim history, a refinement loop may need the previous draft. `llmff` makes that state a declared part of the contract:

```yaml
- id: react_loop
  op: loop
  from: build_initial_context
  max_iterations: 4
  break_on:
    type: field_true
    stage: task_done
    field: passed
  initial_carry:
    history: []
  carry:
    history: updated_history
  final:
    from: final_answer
    require_status: success
  body:
    - id: reason
      op: infer
      from: input
      model: mock:good
      response_format: json

    - id: parse_action
      op: validate_json
      from: reason
      schema: '{"type":"object","required":["tool","args","done"],"properties":{"tool":{"type":"string"},"args":{"type":"object"},"done":{"type":"boolean"},"final_answer":{"type":"string"}}}'

    - id: task_done
      op: predicate
      from: parse_action
      field: done
      mode: truthy

    - id: execute_tool
      op: tool
      from: parse_action
      command: ["python3", "tool-result.py"]

    - id: observe
      op: validate_json
      from: execute_tool
      schema: '{"type":"object","required":["ok"],"properties":{"ok":{"type":"boolean"},"result":{},"error":{"type":"string"}}}'

    - id: updated_history
      op: accumulate
      from: observe
      state_from: history
      mode: append
      limit: 8

    - id: final_answer
      op: extract
      from: parse_action
      field: final_answer
```

This is still finite work with visible state. `history` is not a hidden memory store — it's an explicit carry value, updated by a named body stage, with a declared limit and a declared final output. That's the whole difference between a loop as an execution primitive and a loop as a small agent runtime that happens to live inside your pipeline.

## Retention makes best-of-N inspectable

Best-of-N usually starts life as a hidden list in host code:

```text
candidates = []
for i in range(n):
    candidates.append(call_model(...))
return choose(candidates)
```

That works fine until someone needs to debug why a particular winner was selected, preserve the candidate scores, or compare traces across runs — at which point the list is gone and the evidence with it. `retain_iterations` moves the relevant iteration data into the loop output instead:

```yaml
retain_iterations:
  mode: all
  stages: [score_candidate]
  include_values: true
```

The downstream `select` stage reads from `iterations`, which makes the selection graph-visible. It also makes retention a deliberate artifact choice with a real tradeoff: including values is useful for debugging and evaluation, but it may be too much payload for some runs, and the manifest has to say which way it's going. The constraint is the feature — hidden lists are easy to write and hard to operate.

## Trace context maps body events back to the loop

If `draft` runs five times, a trace event that says only `"stage_id": "draft"` is useless. Loop body events carry their context:

```json
{
  "event": "stage_finished",
  "stage_id": "refine_loop.draft",
  "op": "infer",
  "status": "success",
  "loop_id": "refine_loop",
  "loop_iteration": 2,
  "loop_stage_id": "draft",
  "duration_ms": 2409,
  "total_tokens": 128
}
```

The triple `(loop_id, loop_iteration, loop_stage_id)` is what makes loop observability work. A dashboard can group body events by loop, a post-run debugger can reconstruct iteration order, and a supervisor can compare where the loop actually stopped against the declared `break_on` condition. All of it from metadata — no prompt dump required.

## What loops deliberately do not own

`op: loop` does not plan the next task, decide whether the job should continue after the manifest exits, own memory, schedule background work, approve tools, or choose which provider a tenant may use. Those are real responsibilities with real owners, and the owners live above `llmff`.

What the loop stage owns is a smaller contract: repeat this body graph at most `N` times, stop when this declared condition is satisfied, carry only this declared state, retain only this declared iteration data, and emit trace events that map back to the body. That turns out to be enough. It gives agent hosts, batch jobs, CI checks, and human-in-the-loop systems a supervisable subprocess for repeated LLM work — without asking any of them to surrender orchestration to the runner.

## Try this

Inspect a bounded refinement loop, then run it with a deterministic mock response:

```bash
llmff inspect examples/loops/self-refining-answer-loop.yaml --format json

LLMFF_MOCK_GOOD_RESPONSE='{"answer":"Use llmff for bounded, inspectable LLM pipelines.","confidence":0.93}' \
llmff run examples/loops/self-refining-answer-loop.yaml \
  --trace /tmp/llmff-self-refining-answer.trace.jsonl
```

Do the same with a best-of-N loop:

```bash
llmff inspect examples/loops/best-of-n-sampling+selection-loop.yaml --format json

LLMFF_MOCK_GOOD_RESPONSE='{"candidate":"Candidate answer from a bounded sample.","score":8}' \
llmff run examples/loops/best-of-n-sampling+selection-loop.yaml \
  --trace /tmp/llmff-best-of-n.trace.jsonl
```

The point was never that loops are new. The point is that these loops are bounded, declared, inspectable before execution, and traceable after it.
