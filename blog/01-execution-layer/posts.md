# Companion Posts

1. The missing abstraction in LLM systems is not another agent loop. It is a bounded execution layer: declared graph in, typed artifacts out, process exit code at the boundary.

2. `llmff` should not know why the job exists. It should know exactly what ran.

3. A good LLM pipeline runner should be inspectable before execution and auditable after execution. `inspect.json` is the preflight contract. `trace.jsonl` is execution evidence.

4. Subprocess semantics are a feature for LLM systems. Spawn the runner, watch events, preserve the exit code, collect artifacts, and let the supervisor own policy.

5. The caller owns planning, memory, and policy. `llmff` owns one bounded run: inputs, stages, outputs, traces, checkpoints, result, exit code.
