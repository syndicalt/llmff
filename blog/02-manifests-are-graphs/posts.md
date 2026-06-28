# Companion Posts

1. A manifest is not a config file for a prompt. It is a small typed graph: inputs, stage IDs, dependencies, operations, and outputs.

2. The unit of reproducibility is not the model call. It is the whole declared run: inputs, graph, schemas, backend aliases, outputs, and execution options.

3. Stable stage IDs are how you connect manifests, inspect reports, traces, outputs, and supervisor decisions.

4. `from` is more than a convenience field. It is a graph edge. Once dependencies are explicit, the runner can order, validate, trace, and reject cycles before execution.

5. A valid LLM pipeline graph has a topological order. If a manifest has missing references or cycles, the right failure time is inspect, not after a provider call.
