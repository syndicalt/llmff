# Providers, Tools, And Plugins At The Boundary

`llmff` integrates with models and tools through explicit transports. It should not become the tool-selection policy engine.

[IMAGE PLACEHOLDER: Boundary diagram showing a supervisor above `llmff`, then provider backends, command tools, HTTP tools, and plugin transports below. The `llmff` box should be labeled "declared calls, typed outputs, trace metadata" rather than "policy".]

There's an easy mistake available at every integration point, and it's the same mistake each time: letting the integration become a policy surface. A model backend shows up, so the runner starts deciding which model is best. A tool call appears, so the runner starts choosing tools. A plugin mechanism lands, and suddenly the runner is acting like a marketplace, a permission system, and an agent host all at once.

That's not the shape I want for `llmff`. The runner should cross boundaries cleanly without owning the reason those boundaries get crossed. For providers, tools, and plugins alike, the pattern is the same: explicit transports in, typed stage values out, trace metadata around the edge. The caller owns policy, the manifest declares execution, and `llmff` runs the declared call.

## Provider aliases are runtime wiring

Provider registration happens at the command line, where it's visible:

```bash
llmff run pipeline.yaml \
  --backend openai=https://api.openai.com/v1 \
  --api-key-env openai=OPENAI_API_KEY
```

The manifest then references the backend alias:

```yaml
version: 1
inputs:
  prompt:
    path: question.txt
graph:
  - id: load_prompt
    op: load
    input: prompt

  - id: draft
    op: infer
    from: load_prompt
    model: openai:gpt-4.1-mini
    temperature: 0.2
    max_tokens: 400
outputs:
  answer:
    from: draft
    path: answer.txt
```

The model ID has two parts, and the split is doing real work. `openai` is the backend alias registered for this run; `gpt-4.1-mini` is the provider model ID sent through that backend. Because the alias resolves at the process boundary, a manifest stays portable without hiding which backend will actually serve it — a supervisor can run the same manifest against an OpenAI-compatible endpoint, a local server, or a test fixture by changing process arguments instead of rewriting graph structure.

For a local OpenAI-compatible server:

```bash
llmff run pipeline.yaml \
  --backend local=http://localhost:8000/v1
```

```yaml
- id: draft
  op: infer
  from: load_prompt
  model: local:llama-3.1-8b-instruct
```

For native Ollama registration:

```bash
llmff run pipeline.yaml \
  --ollama ollama=http://localhost:11434
```

```yaml
- id: draft
  op: infer
  from: load_prompt
  model: ollama:llama3.1
```

I want to be clear about what this is not: it's not a model router. It's runtime wiring. The system above `llmff` decides which provider is allowed for the tenant, the budget, the task class, or the approval state. What `llmff` has to be able to do is inspect the graph, resolve the alias, run the call, and record what happened — and `inspect` accepts the same registration flags so all of that resolves before any provider call:

```bash
llmff inspect pipeline.yaml \
  --backend openai=https://api.openai.com/v1 \
  --api-key-env openai=OPENAI_API_KEY \
  --format json
```

A missing backend alias becomes a graph problem caught at preflight, not a late surprise inside an agent loop.

## Tool stages are declared subprocess or HTTP calls

Tools in `llmff` are stages. They don't get special moral status just because they can touch the outside world. A command tool looks like this:

```yaml
graph:
  - id: load_prompt
    op: load
    input: prompt

  - id: normalize
    op: tool
    from: load_prompt
    command: ["/bin/cat"]
```

The command is argv, not a shell string. The serialized parent value goes to stdin, and stdout becomes the stage output. That small contract earns its keep three times over: no hidden shell expansion, exactly one input channel for the tool, and exactly one output channel for `llmff` to capture, validate, trace, and pass downstream.

HTTP tools use the same stage shape with a different transport:

```yaml
graph:
  - id: call_endpoint
    op: tool
    from: render_prompt
    method: POST
    url: http://127.0.0.1:8080/process
    headers:
      content-type: application/json
```

For `POST`, `PUT`, and `PATCH`, the serialized parent value becomes the request body and the response text becomes the stage output. None of this means the runner chooses the endpoint — the manifest names it, and the caller decides whether that manifest is allowed to run in this environment. Tool policy lives above the subprocess boundary, where it belongs.

## Validate both sides of a tool loop

The riskiest tool loop shape is the one most people write first: model text flows straight into a tool call, then untyped tool output flows straight back into the model. The safer shape is deliberately boring — ask the model for a JSON tool request, validate the request, execute the declared tool stage, validate the result, and accumulate only the typed observation the next iteration needs.

The ReAct-style loop example in the repo uses this request contract:

```json
{
  "tool": "direct",
  "args": {},
  "done": true,
  "final_answer": "Use a bounded loop and inspect the trace."
}
```

And the manifest validates both sides of the tool boundary:

```yaml
- id: parse_action
  op: validate_json
  from: reason
  schema: '{"type":"object","required":["tool","args","done"],"properties":{"tool":{"type":"string"},"args":{"type":"object"},"done":{"type":"boolean"},"final_answer":{"type":"string"}}}'

- id: execute_tool
  op: tool
  from: parse_action
  command: ["python3", "tool-result.py"]

- id: observe
  op: validate_json
  from: execute_tool
  schema: '{"type":"object","required":["ok"],"properties":{"ok":{"type":"boolean"},"result":{},"error":{"type":"string"}}}'
```

I should be honest about what this buys. It's a mechanism, not a guarantee that the tool is safe — a command can still be dangerous, an HTTP endpoint can still do the wrong thing, a plugin can still be too broad for the deployment. What `llmff` provides is a narrow place for the supervisor to apply its policy: inspect the manifest, examine the declared transports, approve or reject the run, then preserve the trace and artifacts as evidence.

## Plugins extend transports without owning trust

Plugins are local executable capabilities declared by `llmff-plugin.yaml`. A tool transport plugin can be as small as:

```yaml
name: tool-stdio-cat
version: 0.1.0
capabilities:
  - kind: tool-transport
    name: stdio-cat
    entrypoint: ./bin/stdio-cat
```

The plugin manifest isn't decoration — it's the trust boundary. It names the capability kind, the capability name, and the executable entrypoint, which means the runner can discover it, the supervisor can list it, CI can validate it, and a deployment policy can decide whether that plugin directory is allowed at all. A manifest then uses the transport explicitly:

```yaml
graph:
  - id: tool_call
    op: tool
    from: request
    transport: stdio-cat
```

Plugin backend capabilities follow the same discipline: a backend plugin registers a model alias, the manifest references the alias, and the runner sends a serialized inference request to the plugin process on stdin expecting JSON on stdout. That's enough surface area for an execution substrate. It is deliberately not enough surface area for a policy engine.

## What should be in the trace

Provider and tool stages need observability, but observability doesn't mean payloads in logs. A model-calling stage traces fields like:

```json
{
  "event": "stage_finished",
  "stage_id": "draft",
  "op": "infer",
  "status": "success",
  "model": "openai:gpt-4.1-mini",
  "backend": "openai",
  "provider_model": "gpt-4.1-mini",
  "duration_ms": 842,
  "prompt_tokens": 118,
  "completion_tokens": 64,
  "total_tokens": 182
}
```

A tool stage traces the transport class and target:

```json
{
  "event": "stage_finished",
  "stage_id": "execute_tool",
  "op": "tool",
  "status": "success",
  "tool_kind": "command",
  "tool_target": "python3 tool-result.py",
  "duration_ms": 31
}
```

Those records tell a dashboard what ran, how long it took, which backend alias was used, and which tool class crossed the boundary — everything operations needs, and nothing it shouldn't have. Prompt bodies, tool stdin and stdout, headers, and secrets stay out. Payloads belong in declared outputs and caller-owned artifacts.

## Why this boundary is worth keeping

The temptation to make the runner smarter is real, and it arrives one reasonable-sounding feature at a time. Let it choose the model. Let it pick the tool. Let it infer which plugin is trusted, keep a catalog, learn from past runs, approve the next step. Every one of those can be a legitimate product feature somewhere in the stack — and every one of them, placed inside `llmff`, makes the execution boundary less inspectable.

So the division stays firm. The caller owns why this provider is allowed, which tools are available, and everything around memory, tenant policy, human approval, and scheduling. `llmff` owns the declared graph that actually ran. That's not modesty; it's how the subprocess stays useful to many different hosts at once.

## Try this

Inspect a tool loop before running it:

```bash
llmff inspect examples/loops/react-style-tool-use-loop.yaml --format json
```

Run the same loop with a deterministic mock model response and capture a trace:

```bash
LLMFF_MOCK_GOOD_RESPONSE='{"tool":"direct","args":{},"done":true,"final_answer":"Use a bounded loop and inspect the trace."}' \
llmff run examples/loops/react-style-tool-use-loop.yaml \
  --trace /tmp/llmff-react-style-tool-use.trace.jsonl
```

Then inspect the provider and plugin surfaces directly:

```bash
llmff backends list --format json \
  --backend openai=https://api.openai.com/v1 \
  --ollama ollama=http://localhost:11434

llmff plugins list --plugin-dir examples/plugins --format json
```

That sequence is the whole point. Before the first provider call or tool invocation, a supervisor can see the graph, the registered backends, and the plugin capabilities sitting at the boundary.
