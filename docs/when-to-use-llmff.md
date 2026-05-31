# When To Use llmff

Use `llmff` when you need explicit typed inference sub-pipelines that can be
checked into source control, inspected before execution, run as a subprocess,
and debugged from artifacts after it exits.

The best fit is a bounded pipeline step:

- a CI job that must turn an input file into a declared output artifact;
- an agent supervisor that wants a reliable subprocess for one inference
  workflow;
- a queue worker that needs trace, event, checkpoint, and result files;
- a shell workflow where inline graph composition is easier than writing a
  custom script;
- a test fixture that must run offline with deterministic mock backends.

Do not use `llmff` as an agent framework, model server, scheduler, memory system,
autonomous planner, vector database, multimodal engine, or provider
account manager. Those systems can call `llmff`; they should not be hidden
inside it.

## Decision Checklist

Use `llmff` when most answers are yes:

- The pipeline has clear inputs and outputs.
- The graph can be declared in YAML or an inline graph.
- The caller can supervise a subprocess and preserve its exit code.
- Artifacts such as inspect reports, traces, events, checkpoints, and
  `result.json` are useful evidence.
- The workflow benefits from typed stage boundaries and reproducible manifests.

Choose a different tool when most answers are yes:

- The system needs autonomous task planning or multi-agent coordination.
- The main requirement is hosting a model endpoint.
- The workflow depends on long-lived conversational memory owned by the runner.
- The caller needs global scheduling, queue leasing, or human approval policy
  inside the same runtime.
- The problem is primarily vector indexing or document ingestion rather than
  inference pipeline execution.

## Integration Shape

The recommended production shape is:

```bash
llmff inspect pipeline.yaml --format json
llmff run pipeline.yaml --run-dir .llmff/runs/job-42
```

The caller owns job identity, task policy, retries, provider budgets, and
retention. `llmff` owns the bounded pipeline execution and emits process status,
safe failure kinds, traces, lifecycle events, checkpoints, and declared output
artifacts.

For examples, start with [`docs/cookbook.md`](cookbook.md). For agent hosts,
use [`docs/agent-workflows.md`](agent-workflows.md) and
[`docs/agent-harness-contract.md`](agent-harness-contract.md).
