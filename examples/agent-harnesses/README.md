# Agent Harness Adapter Examples

These examples show how an agent framework can treat `llmff` as a bounded
subprocess runner. The agent owns planning, memory, retry policy, and task
routing. `llmff` owns one manifest execution and writes machine-readable
artifacts under a caller-owned run directory.

The examples intentionally avoid framework imports at module import time. Basic
help and import smoke checks work without installing the OpenAI Agents SDK or
LangGraph:

```bash
python3 examples/agent-harnesses/openai_agents_tool.py --help
python3 examples/agent-harnesses/langgraph_node.py --help
python3 -m py_compile examples/agent-harnesses/openai_agents_tool.py examples/agent-harnesses/langgraph_node.py
```

Both adapters assume a runner contract shaped like:

```bash
llmff run --run-dir <dir> <manifest> --timeout-ms <ms>
```

The adapter materializes a run-scoped manifest from a template containing
`{{LLMFF_INPUT_PATH}}`, writes the agent input under the run directory, then
reads final status from `<dir>/result.json`. Payloads come from manifest output
paths. Lifecycle events remain in `events.jsonl`; they are never streamed to
stdout. The subprocess is also bounded by Python's `subprocess.run(...,
timeout=...)`, so a stuck process cannot outlive the agent tool call
indefinitely.

Template manifests should declare the placeholder where the run-scoped input
path belongs:

```yaml
inputs:
  prompt:
    path: "{{LLMFF_INPUT_PATH}}"
```

## OpenAI Agents SDK Tool

[`openai_agents_tool.py`](openai_agents_tool.py) exposes:

- `LlmffToolConfig`: manifest, run-root, timeout, and binary settings.
- `run_llmff_pipeline(...)`: framework-independent subprocess runner.
- `build_openai_agents_tool(...)`: lazy OpenAI Agents SDK registration using
  `agents.function_tool`.

Install the optional framework only when wiring the example into an OpenAI
Agents SDK app:

```bash
python3 -m pip install openai-agents
```

Example integration after copying or packaging the adapter in your agent app:

```python
from pathlib import Path

from your_app.openai_agents_tool import (
    LlmffToolConfig,
    build_openai_agents_tool,
)

tool = build_openai_agents_tool(
    LlmffToolConfig(
        manifest=Path("pipelines/issue_triage.yaml"),
        run_root=Path(".llmff/agent-runs"),
    )
)
```

## LangGraph Node

[`langgraph_node.py`](langgraph_node.py) exposes:

- `LlmffNodeConfig`: state keys, manifest, run-root, timeout, and binary
  settings.
- `LlmffRunNode`: a callable LangGraph-compatible node.
- `build_langgraph_node(...)`: returns the node callable without importing
  LangGraph.

Install LangGraph only when building a real graph:

```bash
python3 -m pip install langgraph
```

Example integration after copying or packaging the adapter in your agent app:

```python
from pathlib import Path

from langgraph.graph import StateGraph

from your_app.langgraph_node import (
    LlmffNodeConfig,
    build_langgraph_node,
)

graph = StateGraph(dict)
graph.add_node(
    "run_llmff",
    build_langgraph_node(
        LlmffNodeConfig(
            manifest=Path("pipelines/issue_triage.yaml"),
            run_root=Path(".llmff/langgraph-runs"),
            input_key="task",
            output_key="triage",
        )
    ),
)
```

## Operational Notes

- Set `LLMFF_BIN` to point at a development binary, for example
  `LLMFF_BIN=target/debug/llmff`.
- Give each agent call a unique run directory. The examples generate a unique
  ID when the caller does not provide one.
- Treat the subprocess exit code as authoritative. On failure, the adapters read
  the last `run_failed` event from `events.jsonl` and include its failure kind
  and message in the raised exception.
- Do not pass `--events -` from these adapters. Agent frameworks often use
  stdout for tool transport or logs, so lifecycle events must stay file-backed.
