# Companion Posts

1. A loop without a bound is a policy decision.

   `llmff` v1.1 requires `max_iterations` because a supervisor should know the upper bound before any provider call runs.

2. The loop body is not a programming language. It is a repeated DAG.

   Body stages can reference `input`, carry values, and earlier body stages. Repetition belongs to the loop controller.

3. Best-of-N belongs in the traceable graph, not in a hidden Python list.

   `retain_iterations` lets the manifest say which per-iteration stage values are preserved for downstream selection.

4. Carry is explicit state, not memory.

   If a loop needs history, the manifest names the initial value, the updating stage, and the limit. That is very different from giving the runner a memory system.

5. Loop traces need iteration context.

   `(loop_id, loop_iteration, loop_stage_id)` is the difference between a useful trace and five copies of a stage name.

6. Bounded loops are useful because they are constrained.

   The supervisor owns why the work exists. `llmff` owns the finite repeated graph that actually ran.
