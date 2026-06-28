# llmff Deep Dive X Article Series Plan

## Purpose

Create an educational X-native article series that explains `llmff` from first
principles through v1.1. The series should teach the philosophy, mathematics,
runtime mechanics, manifest structure, observability model, and reasoning
behind each major feature without positioning `llmff` as an all-in-one agent
framework.

The core frame:

> `llmff` is a bounded FFmpeg-style execution runner for LLM pipelines. It
> executes declared graphs, emits inspectable artifacts, and gives supervisors
> a reliable subprocess boundary. It does not plan work, own memory, host
> agents, or replace application orchestration.

## Editorial Format

- 10 standalone deep-dive articles.
- Each article should be publishable on X as a long-form post.
- Each article should include 3-6 shorter companion posts that can be posted
  before or after the article.
- Tone: calm systems founder: technical, educational, restrained,
  evidence-first, with enough conviction to explain why the boundary matters.
- Avoid victory-lap language and broad claims about replacing agent
  frameworks.
- Every article should include at least one concrete manifest, trace fragment,
  schema, or command.
- Images are optional. Use them only when they clarify execution shape,
  boundaries, trace structure, or graph math.

## House Voice

The series should sound like a calm systems founder explaining a useful
execution substrate from first principles.

Voice blend:

- 70% senior systems engineer: precise, concrete, mechanism-first.
- 20% founder philosophy: clear conviction about why the boundary matters.
- 10% X-native punch: memorable lines that can stand alone without becoming
  hype.

The voice should feel:

- confident, not triumphant;
- opinionated, not combative;
- educational, not promotional;
- practical, not academic for its own sake;
- founder-led, but grounded in implementation details.

Use short declarative lines for core claims, then back them with mechanics.

Example rhythm:

> The mistake is treating every LLM workflow like it needs a new agent runtime.
>
> Most of the time, you do not need autonomy. You need a bounded execution
> contract: declared inputs, typed stages, explicit outputs, traces, and a
> process your supervisor can trust.
>
> That is the shape of `llmff`.

Preferred phrases:

- "bounded execution contract"
- "declared graph"
- "inspect before you run"
- "the caller owns why; `llmff` owns what ran"
- "the boundary is the product"
- "supervisable subprocess"
- "typed artifacts"
- "finite work, visible state"
- "execution substrate"

Avoid:

- "revolutionary"
- "game-changing"
- "agents are dead"
- dunking on frameworks or competing projects;
- vague "production-ready AI" claims without a mechanism;
- overclaiming reliability, safety, or cost savings;
- implying `llmff` owns planning, memory, scheduling, or tool policy.

## Human Prose Guardrails

The writing should not sound generated. Drafts should read like a founder who
has built the thing, knows the tradeoffs, and is explaining the system to other
builders without theater.

Avoid common LLM tics:

- generic openings like "In today's rapidly evolving landscape";
- "Let's dive in";
- "At its core";
- "This changes everything";
- "The result?";
- "Here's the thing";
- "not just X, but Y" as a default sentence shape;
- repetitive thesis restatements at the start and end of every section;
- stacked abstractions without examples;
- vague adjectives such as robust, seamless, powerful, scalable, innovative,
  transformative, and production-ready unless the mechanism is shown;
- polished filler paragraphs that delay the concrete point;
- rhetorical questions used as transitions;
- forced symmetry in every paragraph;
- concluding every section with a grand lesson.

Prefer human writing habits:

- Start where the real tension is.
- Name the mechanism before naming the value.
- Use specific nouns: manifest, stage ID, trace event, exit code, run
  directory, schema, provider, subprocess.
- Let examples carry claims.
- Keep one idea per paragraph.
- Use short sentences when making a position clear.
- Use longer sentences only when they carry actual technical detail.
- Admit tradeoffs directly.
- Leave some texture in the prose; not every paragraph needs to resolve into a
  slogan.
- Let the author sound present: "I care about this boundary because..." is
  better than institutional fog.

Before publishing any article, run a prose pass:

1. Delete the first paragraph if it is throat-clearing.
2. Replace vague value claims with mechanisms or examples.
3. Remove any sentence that could appear unchanged in a generic AI startup
   essay.
4. Check that every founder-flare line is earned by nearby technical detail.
5. Read the article aloud and cut sentences that sound too polished to be
   spoken by a real person.

Every article should include one founder-flare line: a compact, quotable
sentence that makes the philosophy memorable while staying technically true.
Examples:

- "The runner should not know why the job exists. It should know exactly what
  ran."
- "A loop without a bound is a policy decision."
- "The manifest is where intent becomes an execution contract."
- "The boundary is not a lack of ambition. It is the product shape."

## Series Arc

The series moves from philosophy to mechanics:

1. Why bounded execution matters.
2. Why manifests are graphs, not prompts.
3. How typed stage values make LLM workflows inspectable.
4. How validation and repair create a useful failure boundary.
5. How observability works through traces, events, and inspect reports.
6. How providers, tools, and plugins fit without taking over the core.
7. How loops work without becoming an agent language.
8. How map and batch differ.
9. How supervisors should call `llmff`.
10. What the system is not, and why that boundary is the product.

## Article 1: The Execution Layer LLM Systems Were Missing

**Thesis:** Most LLM applications mix planning, prompting, validation, retries,
tool calls, and logging in one runtime. `llmff` separates execution from
orchestration: one declared graph in, typed artifacts out.

**Tone note:** Highest founder flare in the series. Define the category with
conviction, then immediately ground it in subprocess mechanics.

**Core ideas:**
- `llmff` is closer to FFmpeg than to an agent framework.
- The application or agent host owns intent, memory, planning, and policy.
- `llmff` owns a bounded run: inputs, stages, outputs, exit code, trace.
- Subprocess semantics are a feature, not a limitation.

**Mechanics to show:**
```bash
llmff inspect pipeline.yaml --format json
llmff run --run-dir .llmff/runs/job-42 pipeline.yaml
```

**Reasoning to explain:**
- Why a process boundary gives supervisors a clean control surface.
- Why exit codes and artifact files are easier to operate than framework
  callbacks.
- Why "boring" matters for production LLM workflows.

**Companion posts:**
1. "The missing abstraction in LLM systems is not another agent loop. It is a
   bounded execution layer."
2. "`llmff` should not know why the job exists. It should know exactly what ran."
3. "A good LLM pipeline runner should be inspectable before execution and
   auditable after execution."

**Image idea:** Layer diagram: supervisor/application above, `llmff` execution
runner in the middle, providers/tools/files below. Highlight the process
boundary.

## Article 2: Manifests Are Graphs, Not Prompt Wrappers

**Thesis:** A prompt is a string. A pipeline is a graph of typed operations.
`llmff` manifests make dependencies explicit so the runtime can inspect,
validate, order, trace, and reproduce the run.

**Tone note:** More engineer than founder. Use the founder voice only to make
the claim that the manifest is where intent becomes an execution contract.

**Core ideas:**
- A manifest declares inputs, graph stages, and outputs.
- Stage IDs are stable handles for execution, tracing, and supervision.
- `from` creates data dependencies.
- Execution can be dependency ordered instead of written in linear script form.

**Mechanics to show:**
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
    model: mock:good
outputs:
  final:
    from: draft
    path: answer.txt
```

**Math/structure angle:**
- Treat the manifest as a DAG `G = (V, E)`.
- Stages are vertices; `from`, route targets, body references, and outputs are
  edges.
- A valid graph has a topological order.
- Static validation rejects missing references and cycles before the run.

**Companion posts:**
1. "A manifest is not a config file for a prompt. It is a small typed graph."
2. "The unit of reproducibility is not the model call. It is the whole declared
   run."
3. "Stable stage IDs are how you connect manifests, traces, outputs, and
   supervisor decisions."

**Image idea:** DAG visualization with `load -> template -> infer ->
validate_json -> write`, with stage IDs shown as trace handles.

## Article 3: Typed Values, Validation, And The Cost Of Ambiguity

**Thesis:** LLM workflows fail when everything is "just text." `llmff` uses
typed stage values and JSON Schema validation to make output contracts
explicit.

**Tone note:** Teach through failure modes. Keep the founder flare pointed at
the cost of ambiguity, not at broad claims about correctness.

**Core ideas:**
- Stages produce typed values: text, JSON, messages, success, invalid, skipped.
- `validate_json` does not have to crash the whole run; it can produce an
  invalid status that routing or repair can handle.
- Structured outputs make traces and downstream tooling more useful.

**Mechanics to show:**
```yaml
- id: draft
  op: infer
  from: prompt
  model: openai:gpt-4.1-mini
  response_format: json

- id: validate
  op: validate_json
  from: draft
  schema: '{"type":"object","required":["answer"]}'
```

**Math/structure angle:**
- A stage is a partial function:
  `stage: InputValue -> Result<StageStatus, StageError>`.
- `StageStatus` separates semantic invalidity from execution failure.
- This distinction lets the graph encode recovery instead of burying it in
  exception handling.

**Companion posts:**
1. "Text is the worst possible interface between two LLM workflow stages."
2. "Invalid JSON is not always a runtime failure. Sometimes it is a typed state
   the graph should route around."
3. "Validation is where an LLM pipeline becomes an interface."

**Image idea:** Type-state diagram: `Success(JSON)`, `Invalid(errors, value)`,
`Skipped`, `StageExecutionError`.

## Article 4: Repair, Route, And Explicit Failure Paths

**Thesis:** Production LLM systems need recoverable failure paths. `llmff`
models repair and routing as declared graph stages, not hidden retry logic.

**Tone note:** Practical operator voice. Show respect for messy real-world
LLM outputs, then explain why explicit failure paths are a product decision.

**Core ideas:**
- `repair` is a model call with a specific job: fix invalid structured output.
- `route` chooses among already-computed stage outputs by status or field.
- `when` guards stages by parent status.
- Error handling belongs in the manifest when it is part of the workflow.

**Mechanics to show:**
```yaml
- id: validate
  op: validate_json
  from: draft
  schema: ./answer.schema.json

- id: repair
  op: repair
  from: validate
  model: mock:good

- id: choose
  op: route
  from: validate
  on_success: validate
  on_invalid: repair
```

**Reasoning to explain:**
- Difference between transport retries and semantic repair.
- Why hidden retry loops make observability worse.
- Why declared routes make failure modes testable.

**Companion posts:**
1. "Retries are for flaky transport. Repair is for bad semantics. They are not
   the same operation."
2. "A route stage is a production incident waiting to not happen."
3. "If a failure path matters, it belongs in the manifest."

**Image idea:** Flowchart with invalid JSON going through repair, valid JSON
going straight to output.

## Article 5: Inspect Before You Run

**Thesis:** `inspect` is the preflight contract. It tells a supervisor what
will run, what models are referenced, what outputs are owned, and what bounds
exist before any provider call happens.

**Tone note:** Founder flare should focus on trust before execution. The
article should feel like an argument for operational discipline.

**Core ideas:**
- `inspect --format json` is for machines.
- It reports graph order, schema compatibility, model aliases, plugin metadata,
  stdout ownership, loop/map bounds, and execution controls.
- Inspect reports are artifacts that belong next to trace and output files.

**Mechanics to show:**
```bash
llmff inspect examples/loops/self-refining-answer-loop.yaml --format json
```

Loop metadata to highlight:
```json
{
  "max_iterations": 5,
  "body_stage_count": 5,
  "max_expanded_stage_count": 25
}
```

**Math/structure angle:**
- Bounds are static estimates:
  `max_expanded_stage_count = body_stage_count * max_iterations`.
- For map:
  `max_expanded_stage_count = body_stage_count * max_items`.
- These are upper bounds, not predictions.

**Companion posts:**
1. "The safest LLM call is the one your supervisor inspected before it ran."
2. "Cost control starts with static bounds."
3. "An inspect report is a contract between a pipeline author and a runner."

**Image idea:** Inspect report card with graph order, output ownership, loop
bounds, and backend aliases.

## Article 6: Traces, Events, And Observability Without Prompt Logging

**Thesis:** LLM pipelines need observability, but observability should not mean
dumping prompts and secrets into logs. `llmff` traces safe metadata by default.

**Tone note:** Calm and precise. Avoid fear-based security language; explain
why metadata-first observability is the right default.

**Core ideas:**
- Trace/event streams are metadata, not payload logs.
- Stage events include status, duration, provider metadata, usage, failure
  kind, loop context, and map context.
- Payloads belong in declared outputs and caller-owned artifacts.

**Mechanics to show:**
```json
{
  "event": "stage_finished",
  "stage_id": "live_refine.draft",
  "op": "infer",
  "status": "success",
  "loop_id": "live_refine",
  "loop_iteration": 1,
  "loop_stage_id": "draft",
  "duration_ms": 2409,
  "total_tokens": 128
}
```

**Reasoning to explain:**
- Why trace is safe metadata.
- How traces support dashboards and post-run debugging.
- Why failure kinds matter for supervisors.

**Companion posts:**
1. "A trace should tell you what happened without becoming a prompt dump."
2. "Loop observability needs iteration context, not log scraping."
3. "Stage IDs are the bridge between manifests and traces."

**Image idea:** JSONL trace timeline with nested loop events and duration bars.

## Article 7: Providers, Tools, And Plugins At The Boundary

**Thesis:** `llmff` integrates with models and tools through explicit
transports. It should not become the tool-selection policy engine.

**Tone note:** Boundary-forward. The founder voice should make the refusal to
own tool policy feel deliberate and useful.

**Core ideas:**
- Model aliases resolve to backend registrations.
- OpenAI-compatible and Ollama backends are runtime adapters.
- `tool` stages call command, HTTP, or plugin transports.
- Supervisors own tool catalog policy; `llmff` executes declared calls.

**Mechanics to show:**
```bash
llmff run pipeline.yaml \
  --backend openai=https://api.openai.com/v1 \
  --api-key-env openai=OPENAI_API_KEY
```

Tool loop contract:
```json
{
  "tool": "lookup",
  "args": { "query": "..." },
  "done": false,
  "final_answer": null
}
```

**Reasoning to explain:**
- Why command tools receive serialized parent values on stdin.
- Why tool output should be validated before accumulation.
- Why plugin capability manifests are explicit trust boundaries.

**Companion posts:**
1. "`llmff` can run a tool. It should not decide which tools your product is
   allowed to use."
2. "A command tool is just another explicit stage with stdin/stdout semantics."
3. "Provider aliases keep manifests portable without hiding the backend."

**Image idea:** Boundary diagram: model backend, HTTP tool, command tool, plugin
transport feeding typed stage outputs back into the graph.

## Article 8: Bounded Loops Without Becoming An Agent Framework

**Thesis:** v1.1 adds loops, but it does not add autonomous agents. A loop is a
bounded stage with an embedded body graph, explicit break condition, and
traceable per-iteration execution.

**Tone note:** This is the flagship technical essay. Use a strong hook, then
be careful and exact. The article should make bounded loops feel powerful
because they are constrained.

**Core ideas:**
- `max_iterations` is required.
- `break_on` is required.
- Loop body is still a DAG.
- `carry` is explicit.
- `retain_iterations` enables best-of-N selection without hidden state.

**Mechanics to show:**
```yaml
- id: sample_loop
  op: loop
  from: build_prompt
  max_iterations: 2
  break_on:
    type: never
  retain_iterations:
    mode: all
    stages: [score_candidate]
    include_values: true
  body:
    - id: draft
      op: infer
      from: input
      model: openai:gpt-4.1-mini
```

**Math/structure angle:**
- A bounded loop is a finite unrolling of a subgraph:
  `Loop(G_body, N) -> G_body^1 ... G_body^N`.
- Break conditions make execution shorter than the upper bound but never longer.
- Trace context maps each body event back to `(loop_id, iteration, stage_id)`.

**Companion posts:**
1. "A loop without a bound is a policy decision. `llmff` requires the bound."
2. "The loop body is not a programming language. It is a repeated DAG."
3. "Best-of-N belongs in the traceable graph, not in a hidden Python list."

**Image idea:** Unrolled loop diagram showing three copies of the body DAG and
a break condition gate.

## Article 9: Map, Batch, And The Math Of Bounded Fan-Out

**Thesis:** `op: map` and CLI batch mode solve different fan-out problems.
Map applies a bounded body graph inside one run. Batch runs the whole manifest
once per input item with separate artifacts.

**Tone note:** Mechanism-first. Founder flare should be about deterministic
fan-out, not throughput hype.

**Core ideas:**
- `op: map` requires `items_from`, `max_items`, and `body`.
- Map body gets reserved `item`.
- Parallel map requires `parallel: true` and `max_concurrency`.
- Output order stays deterministic by item index.
- Batch mode is for independent job items and per-item artifact isolation.

**Mechanics to show:**
```yaml
- id: map_names
  op: map
  from: load_payload
  items_from: items
  max_items: 3
  parallel: true
  max_concurrency: 2
  body:
    - id: name
      op: extract
      from: item
      field: name
```

**Math/structure angle:**
- Static upper bound:
  `map_work <= min(len(items), max_items) * body_stage_count`.
- Parallelism changes scheduling, not output semantics.
- Deterministic order is by input index, not completion time.

**Companion posts:**
1. "Map is in-pipeline fan-out. Batch is job-level fan-out."
2. "Parallel execution should not make output order nondeterministic."
3. "A bounded map is a finite collection transform, not a scheduler."

**Image idea:** Side-by-side: one run with map over items vs many independent
batch item runs.

## Article 10: The Boundary Is The Product

**Thesis:** The most important design choice in `llmff` is what it refuses to
own. It does not plan, remember, schedule, approve, or host agents. It executes
declared bounded pipelines well enough for other systems to supervise.

**Tone note:** Philosophical closing essay. Highest-level product argument,
but every claim should tie back to a concrete interface: manifests, traces,
exit codes, artifacts, and schemas.

**Core ideas:**
- Explicit non-goals protect the core.
- PM layers, memory systems, human approval, and autonomous planning sit above.
- Backends, tools, files, and plugins sit below.
- `llmff` is the reproducible execution substrate in the middle.

**Mechanics to show:**
Supervisor sequence:
```text
inspect -> run -> preserve exit code -> store trace/events/output -> decide next step
```

**Reasoning to explain:**
- Boundary discipline keeps the runner testable.
- A narrow execution layer composes better with many hosts.
- A clear non-goal is an operational feature.

**Companion posts:**
1. "The boundary is not a lack of ambition. It is the product shape."
2. "`llmff` does not need to be your agent framework to be useful to agents."
3. "The caller owns why. `llmff` owns what ran."

**Image idea:** Clean architecture stack with "why" above, "what ran" in
`llmff`, and "how calls execute" below.

## Cross-Series Reusable Examples

Use these recurring artifacts to make the series coherent:

- A small JSON repair manifest for validation/repair/routing articles.
- A self-refining answer loop for loop/predicate/extract articles.
- A ReAct-style tool loop for tool boundary and accumulation articles.
- A best-of-N sampling loop for retention/score/select articles.
- A map batch items manifest for map/batch distinction articles.
- A live trace excerpt from `openai:gpt-4.1-mini` showing token usage and loop
  context.

## Suggested Image Prompt Templates

Use generated images sparingly. Prefer diagrams over decorative art.

### Execution Boundary

> Technical architecture diagram, clean white background, three horizontal
> layers. Top layer labeled "Supervisor / agent host: planning, memory, policy".
> Middle layer labeled "llmff: bounded manifest execution, traces, exit codes".
> Bottom layer labeled "Models, tools, files, plugins". Use thin lines and
> precise labels, no mascot, no marketing style.

### Graph Manifest

> Minimal DAG diagram of an LLM pipeline: load -> template -> infer ->
> validate_json -> route -> write. Each node has a small stage ID label.
> Include a side panel showing "manifest", "trace", and "output" linked by the
> same stage IDs.

### Loop Trace

> Diagram showing a bounded loop unrolled into three iterations. Each iteration
> contains draft -> validate -> predicate. Include trace labels loop_id,
> loop_iteration, loop_stage_id. Clean educational style.

### Map Vs Batch

> Side-by-side technical diagram. Left: one pipeline run with op: map over
> items 0,1,2 inside the graph. Right: batch mode with three separate pipeline
> runs and isolated output folders. Use neutral colors and concise labels.

## Drafting Rules For Each Article

Each article should include:

1. A one-sentence thesis.
2. A concrete problem in LLM systems.
3. The `llmff` mechanism.
4. A manifest or trace snippet.
5. The reasoning behind the design.
6. What `llmff` deliberately does not own.
7. A short "try this" command or example path.

Avoid:

- claiming `llmff` replaces agent frameworks;
- saying "workflow engine" without clarifying the bounded execution boundary;
- presenting loops as autonomous;
- showing invalid manifest fields such as `prompt:` on `infer`;
- implying traces contain prompt payloads by default;
- making broad cost or reliability claims without a concrete mechanism.

## Production Checklist For The Series

- [ ] Article 1 draft: execution layer philosophy.
- [ ] Article 2 draft: manifests as DAGs.
- [ ] Article 3 draft: typed values and validation.
- [ ] Article 4 draft: repair, route, and explicit failure paths.
- [ ] Article 5 draft: inspect reports and static bounds.
- [ ] Article 6 draft: traces, events, and observability.
- [ ] Article 7 draft: providers, tools, and plugins.
- [ ] Article 8 draft: bounded loops.
- [ ] Article 9 draft: map vs batch.
- [ ] Article 10 draft: boundary as product.
- [ ] Generate or draw diagrams only for articles where a diagram clarifies the
  concept.
- [ ] Run every manifest snippet through `llmff inspect` or mark it explicitly
  as pseudocode.
- [ ] Keep companion posts factual and connected to the article, not hype.
