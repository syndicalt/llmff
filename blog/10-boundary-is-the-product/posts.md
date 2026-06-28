# Companion Posts

1. The boundary is not a lack of ambition. It is the product shape.

`llmff` does not plan, remember, schedule, approve, or host agents.

It executes declared bounded pipelines and leaves artifacts another system can supervise.

2. The caller owns why. `llmff` owns what ran.

That split keeps the runner usable from agent hosts, CI jobs, queue workers, and shell scripts without forcing all of them into one framework.

3. A good supervisor sequence is boring:

`inspect -> run -> preserve exit code -> store trace/events/output -> decide next step`

The decision belongs above the subprocess boundary.

4. Non-goals are operational features.

If the runner does not own memory, inputs stay explicit.

If it does not own scheduling, queues keep their leases and retries.

If it does not own approval, products keep their permission model.

5. `llmff` does not need to be your agent framework to be useful to agents.

An agent can plan the work, materialize inputs, call `llmff` as a subprocess, read the result artifacts, and decide the next step.

That is enough.

6. The product bet is narrow:

declared graph in,
typed artifacts out,
inspect before execution,
trace after execution,
exit code at the boundary.
