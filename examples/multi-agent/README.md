# Multi-Agent Topology Examples

These examples show how to declare a **multi-agent topology** in a single
manifest. An `agent` here is a reusable bundle of a persona (`system`), a model,
and sampling settings. A stage references one with `agent: <name>`, and the
reference is expanded at inspect time into concrete inference fields — the graph
stays a bounded, declared DAG.

This does not make `llmff` an agent orchestrator. There is no dynamic dispatch
and no agent that picks the next agent at runtime; the topology is fully
declared and inspectable. The host above still owns why the pipeline runs and
what happens next. See the "Declared Multi-Agent Topology Boundary" section of
[`SPEC.md`](../../SPEC.md).

The mock backend returns one configured response for every model call in a run,
so these examples prove the topology, role-stamped tracing, and inspect
metadata rather than distinct per-role text. To use them with a real provider,
replace `model: mock:good` in each agent with a registered provider model such
as `openai:gpt-4.1-mini` and run with
`--backend openai=https://api.openai.com/v1 --api-key-env openai=OPENAI_API_KEY`.

## Generator / Critic / Reviser

Three declared roles wired into one bounded refine loop: a `generator` drafts an
answer, then each iteration a `critic` reviews it and a `reviser` applies the
critique. The loop stops as soon as the critic accepts, and never runs more than
`max_iterations`. Each role's name is stamped onto its stage trace events, so a
supervisor can attribute work per role from the JSONL trace.

```bash
llmff inspect examples/multi-agent/generator-critic-reviser.yaml
LLMFF_MOCK_GOOD_RESPONSE='{"answer":"Set max_iterations and break on a success predicate.","accept":true,"issues":[]}' \
llmff run examples/multi-agent/generator-critic-reviser.yaml \
  --trace /tmp/llmff-generator-critic-reviser.trace.jsonl
```

Operations shown: `agents:` role bundles, per-stage `agent:` references, `loop`,
`validate_json`, `predicate`, and per-role `agent` fields in trace events.

## Planner / Executor

Two declared roles in a straight pipeline: a `planner` decomposes the task into
steps, then an `executor` carries the plan out. Each role's output is validated
against a typed schema before the next role runs.

```bash
llmff inspect examples/multi-agent/planner-executor.yaml
LLMFF_MOCK_GOOD_RESPONSE='{"steps":["Outline the bound.","Write the guard."],"answer":"Bound the loop with max_iterations and a success predicate.","status":"complete"}' \
llmff run examples/multi-agent/planner-executor.yaml \
  --trace /tmp/llmff-planner-executor.trace.jsonl
```

Operations shown: `agents:` role bundles, per-stage `agent:` references, and
`validate_json` between roles.

## Debate / Judge

Three declared roles converging on a decision: an `advocate` argues for a claim,
a `skeptic` argues against it, and a `judge` weighs both and returns a verdict.
The judge reads the arguments, but the wiring is fully declared — no role spawns
debaters or chooses who argues next.

```bash
llmff inspect examples/multi-agent/debate-judge.yaml
LLMFF_MOCK_GOOD_RESPONSE='{"argument":"Bounds make runs predictable.","winner":"advocate","rationale":"The bounded design is safer."}' \
llmff run examples/multi-agent/debate-judge.yaml \
  --trace /tmp/llmff-debate-judge.trace.jsonl
```

Operations shown: multiple `agent:` roles feeding a final decision role, plus
`validate_json` on the verdict.

## Triage / Specialist Handoff

A bounded handoff. A `triage` role labels the request, and `route` selects the
matching specialist's output from a **declared, finite** set of agent stages.
This is the in-scope form of multi-agent handoff: the target set is fixed in the
manifest and visible to `inspect`. It is not dynamic dispatch — no role invents
a new target or chooses an undeclared successor at runtime. Under the mock
backend every declared specialist runs and `route` returns the matching one's
output; with a real provider you would gate or accept that cost above llmff.

```bash
llmff inspect examples/multi-agent/triage-specialist-handoff.yaml
LLMFF_MOCK_GOOD_RESPONSE='{"route":"billing","answer":"Your duplicate charge will be refunded in 3-5 days.","category":"billing"}' \
llmff run examples/multi-agent/triage-specialist-handoff.yaml \
  --trace /tmp/llmff-triage-specialist-handoff.trace.jsonl
```

Operations shown: `agents:` role bundles, per-stage `agent:` references,
`validate_json`, and `route` over a declared, finite set of named agent stages.
