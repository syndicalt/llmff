# Typed Values, Validation, And The Cost Of Ambiguity

[IMAGE PLACEHOLDER: Type-state diagram showing a stage output becoming `Success(Json)`, `Invalid(value, errors)`, or `Skipped`, with execution errors shown outside the typed status path.]

The cheapest mistake in an LLM workflow is calling everything text. It feels simple at first — a prompt goes in, a string comes out, the next stage receives the string, trims it, maybe parses it, maybe just hopes the last model followed the instruction. Then the workflow gets a second stage, and a schema, and a supervisor, and a retry path, and eventually a production incident where the model returned a paragraph that looked correct to every human who read it and still failed as an interface.

Text is a fine format for people. It's a bad boundary between workflow stages.

`llmff` treats stage outputs as typed values with typed status, and that one choice changes how validation works. A JSON validation miss doesn't have to disappear into a thrown exception, a log line, or a hand-rolled retry loop — it can become a visible state in the graph, one that routing and repair can see and act on. The short version of the argument: ambiguity compounds until the graph names it.

## The Value Is Not The Status

`llmff` separates the value a stage produced from the status of producing it. The value shape is deliberately small:

```rust
pub enum Value {
    Text(String),
    Messages(Vec<Message>),
    Json(serde_json::Value),
}
```

The status shape lives apart from it:

```rust
pub enum StageStatus {
    Success(Value),
    Invalid { value: Value, errors: Vec<String> },
    Skipped,
}
```

Keeping those separate matters more than it looks. A stage can produce a perfectly well-formed JSON object that is still semantically invalid for this workflow. A guarded stage can be skipped without pretending it failed. A model call can fail at the transport layer without being confused with "the JSON was missing `answer`." Those are different facts, and a runner that collapses them into one bucket forces every consumer downstream to reconstruct the difference from log text.

In a less typed runner, a downstream stage might receive any of these:

```text
{"answer":"ok"}
```

```text
I think the answer is probably ok.
```

```text
{"wrong":true}
```

All three are strings until some later code decides otherwise. In `llmff`, the graph says what it expects up front:

```yaml
version: 1
inputs:
  prompt:
    path: ./question.txt
graph:
  - id: load_prompt
    op: load
    input: prompt

  - id: draft
    op: infer
    from: load_prompt
    model: openai:gpt-4.1-mini
    response_format: json

  - id: validate
    op: validate_json
    from: draft
    schema: '{"type":"object","required":["answer"],"properties":{"answer":{"type":"string"}},"additionalProperties":false}'
outputs:
  final:
    from: validate
    path: ./answer.json
```

One distinction worth slowing down for: `response_format: json` and `validate_json` look related, but they're different operations doing different jobs. The first is a provider-facing hint — it asks the backend for a JSON-shaped answer when the backend supports that mode, and it makes the output easier to use. The second is the workflow contract: it checks the produced value against a JSON Schema and decides whether this workflow accepts it. The hint improves your odds. The schema is the interface.

## Validation Should Produce Information

A validator whose only move is crashing the run is too blunt for how LLMs actually fail.

There are two cases, and they deserve different treatment. If the schema itself is unreadable, that's a stage execution problem — the manifest author handed the runner an invalid contract, and the run should fail. But if the model returned valid JSON with the wrong fields, the stage did its job: it produced a value, evaluated it, and found it wanting. The graph may want to repair that value, route around it, or preserve it as evidence. None of those options exist if validation only knows how to throw.

`llmff` keeps the difference visible. Given this schema:

```json
{
  "type": "object",
  "required": ["answer"],
  "properties": {
    "answer": {
      "type": "string"
    }
  },
  "additionalProperties": false
}
```

and this model output:

```json
{"wrong": true}
```

the validation stage finishes as invalid rather than pretending no value exists:

```json
{
  "event": "stage_finished",
  "stage_id": "validate",
  "op": "validate_json",
  "status": "invalid",
  "duration_ms": 1,
  "validation_errors": ["\"answer\" is a required property"]
}
```

The exact wording of validation errors varies with the schema library and the schema shape, so don't build on the strings. The stable contract is the structure: the trace records `status: "invalid"` with validation metadata, and the stage status still carries the rejected value. That's the handle the rest of the graph gets to hold.

## The Stage As A Partial Function

The way I think about a stage is as a partial function:

```text
stage: InputValue -> Result<StageStatus, StageError>
```

The signature isn't decoration — it says there are two distinct axes of outcome. `StageStatus` describes the workflow-level result: `Success(value)`, `Invalid(value, errors)`, or `Skipped`. `StageError` describes execution failure: a missing file, an unknown backend, an invalid inline schema, an HTTP 500 that survived the retry policy, a tool exiting non-zero, a timeout.

The runner must not flatten those into "failed," because they demand different responses. If the manifest refers to a missing schema file, the stage never executed its contract and the supervisor should see a failed run. If the model produced `{"wrong":true}` against a schema requiring `answer`, the stage executed its contract completely — it found the value invalid — and the graph gets to decide what happens next.

That's the boundary between runtime failure and semantic invalidity, and it isn't academic. It decides whether repair stages, routing, traces, dashboards, and supervisors can all agree on what occurred.

## The Manifest Makes The Interface Explicit

A schema sitting next to a prompt is only documentation until the runner enforces it. Here's the common shape, with the schema in its own file:

```yaml
version: 1
inputs:
  prompt:
    path: ./question.txt
graph:
  - id: load_prompt
    op: load
    input: prompt

  - id: draft
    op: infer
    from: load_prompt
    model: mock:bad
    response_format: json

  - id: validate
    op: validate_json
    from: draft
    schema_path: ./answer.schema.json
outputs:
  final:
    from: validate
    path: ./answer.json
```

```json
{
  "type": "object",
  "required": ["answer"],
  "properties": {
    "answer": {
      "type": "string"
    }
  },
  "additionalProperties": false
}
```

The value path is inspectable before the run, auditable during it, and summarizable after:

```bash
llmff inspect pipeline.yaml
llmff run pipeline.yaml --trace .llmff/runs/job-42/trace.jsonl
llmff trace .llmff/runs/job-42/trace.jsonl
```

This is the practical difference between "we asked for JSON" and "the run contains a validation stage with a named schema and a recorded status." The first is a hope. The second is a system interface.

## Messages Are Values Too

Typed values aren't only about JSON. `llmff` also has a `Messages` value for chat-shaped content, and a `system` stage can preserve roles by turning file contents into a system message and the parent value into a user message:

```yaml
graph:
  - id: render_prompt
    op: template
    from: load_prompt
    path: ./prompt.tmpl

  - id: apply_policy
    op: system
    from: render_prompt
    path: ./policy.md

  - id: draft
    op: infer
    from: apply_policy
    model: openai:gpt-4.1-mini
    response_format: json
```

This keeps a policy prompt from degenerating into an anonymous string concatenated onto another anonymous string — the role structure survives all the way to the provider call. Some stages genuinely need text, and `llmff` renders messages conservatively for those cases. The goal isn't to make every boundary complex; it's to keep the real shape available wherever the runtime can use it.

## The Trace Gets Better When Values Have Types

Untyped logs drift toward payload dumps, and I think the reason is simple: when the runner has no vocabulary for what happened, the only thing it can write down is everything.

Typed stages give traces a vocabulary, which lets them stay smaller and safer at the same time. A model stage reports model and token metadata. A validation stage reports validation errors. A skipped stage says it was skipped. None of that requires logging the full prompt or tool payload:

```json
{"event":"stage_finished","stage_id":"draft","op":"infer","status":"success","duration_ms":2409,"model":"openai:gpt-4.1-mini","backend":"openai","total_tokens":128}
{"event":"stage_finished","stage_id":"validate","op":"validate_json","status":"invalid","duration_ms":1,"validation_errors":["missing answer"]}
```

Two lines, and a supervisor can already distinguish provider behavior from schema behavior. A dashboard can count invalid structured outputs without parsing prose. A human knows where to look next.

## Ambiguity Has Operational Cost

When every stage boundary is text, each downstream consumer pays a parsing tax. The template stage has to guess, the router has to guess, the repair prompt has to guess, and the supervisor has to infer from logs. Worse, the product code around the runner starts accumulating small private conventions: "this model usually returns JSON," "this field is optional unless the prompt says otherwise," "retry when parse fails," "repair if the string starts with a paragraph." Every one of those conventions works until it doesn't, and none of them can be inspected before a run or audited after one.

Typed artifacts make the contract boring, which is exactly the property you want:

```text
load_prompt -> Success(Text)
apply_policy -> Success(Messages)
draft -> Success(Json)
validate -> Invalid(Json, errors)
repair -> Success(Json)
choose_final -> Success(Json)
```

That's the shape supervisors need. The caller still owns why the job exists, and the product still decides whether invalid output should be repaired, rejected, escalated, or saved for review. `llmff` owns what ran and what typed state each stage produced. The division of labor is the whole design.

## Validation Is Where A Pipeline Becomes An Interface

An LLM call by itself is a guess wrapped in transport. A validated stage is an interface: it has a named input, a stage ID, a schema, a status, and a trace record, and it can be inspected before execution and audited after.

None of that makes the model correct. It makes the failure legible, which is the realistic goal. LLMs will keep returning malformed content, incomplete objects, polite prose where JSON was requested, extra fields, and near-misses — a runner that pretends otherwise is lying to its operators. The useful move is to stop hiding the ambiguity: make values typed, make validation explicit, and let invalid be a first-class state. Then the next stage has something real to work with.
