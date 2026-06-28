# Companion X Posts

1. The safest LLM call is the one your supervisor inspected before it ran.

`llmff inspect --format json` gives the caller a preflight contract: stage order, model aliases, plugin metadata, stdout ownership, artifact paths, and loop/map bounds.

2. Cost control starts with static bounds.

For a loop:

```text
max_expanded_stage_count = body_stage_count * max_iterations
```

That is not a token estimate. It is a structural ceiling available before the first provider call.

3. An inspect report is a contract between a pipeline author and a runner.

The author declares the graph.
The runner reports what it can execute.
The supervisor decides whether that work is allowed in this context.

4. Stdout needs one owner.

Events, streamed stage payloads, and manifest outputs are different protocols. `llmff` exposes stdout ownership in inspect reports and rejects conflicting stream layouts.

5. The caller owns why the job exists. `llmff` owns what ran.

`inspect.json` is the handshake before execution. `trace.jsonl`, `events.jsonl`, and the exit code are the evidence after execution.

