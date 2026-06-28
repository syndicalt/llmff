# Repair, Route, And Explicit Failure Paths

[IMAGE PLACEHOLDER: Flowchart showing `draft -> validate`; valid output routes directly to `choose_final`, invalid output passes through `repair`, then `choose_final` writes the selected JSON.]

LLM output gets messy in ordinary, predictable ways. The model returns an object with the wrong field. It wraps perfectly good JSON in a paragraph of prose. It produces a valid shape with one extra key your downstream system rejects. It answers the question, just not in the interface you declared. Anyone who has run these systems for more than a week has seen all of it, and none of it is surprising.

The mistake isn't that these failures happen. The mistake is hiding the recovery path in code nobody can inspect before the run. `llmff` takes the opposite position: repair and routing are graph stages — declared operations with stage IDs, dependencies, statuses, and trace records — not secret retry loops buried in a client wrapper.

If a failure path matters, it belongs in the manifest. That sentence is the product decision this article is about.

## Retry And Repair Are Different Operations

A transport retry asks one question: did the call fail for a temporary execution reason? A repair stage asks a different one: did the model produce a value that failed the workflow contract, and can a focused model call transform it into an acceptable one? Conflating the two is how systems end up retrying a semantic failure three times with an identical prompt and paying for three identical wrong answers.

For transport, a retry policy belongs near the execution controls:

```bash
llmff run --retry-attempts 3 --retry-backoff-ms 250 pipeline.yaml
```

or on a stage:

```yaml
retry:
  attempts: 3
  backoff_ms: 250
```

That handles failed model attempts and retryable HTTP tool failures. But it has nothing useful to say about this:

```json
{"wrong": true}
```

against this:

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

The call succeeded. The value failed the interface. That's repair territory.

## The Small Repair Graph

The basic repair shape is compact:

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

  - id: repair
    op: repair
    from: validate
    when: invalid
    model: mock:good
    response_format: json

  - id: choose_final
    op: route
    from: validate
    on_success: validate
    on_invalid: repair
outputs:
  final:
    from: choose_final
    path: ./answer.json
```

Three details in this graph carry most of the design.

First, `repair` reads from `validate`, not from `draft`. That means the repair stage receives the invalid status, the rejected value, and the validation errors together — everything it needs for its narrow job, which is to produce a value that satisfies the declared contract. It isn't re-answering the question; it's fixing an interface violation with the violation report in hand.

Second, `when: invalid` keeps repair from running when validation succeeded. On the happy path, the repair stage is marked skipped before any model-specific work happens. It still gets a traceable status, but it doesn't spend tokens pretending to improve a value that already matched the schema.

Third, `choose_final` routes by the status of `validate`: success selects `validate`, invalid selects `repair`. The final output depends on a named choice between named values, not on whichever exception handler happened to fire.

## What The Run Looks Like

Run the pipeline with a trace:

```bash
llmff run examples/json-repair.yaml --trace /tmp/llmff-trace.jsonl
```

With a bad draft and a successful repair, the stage story reads cleanly:

```text
load_prompt   success
draft         success
validate      invalid
repair        success
choose_final  success
```

And the trace carries the same shape:

```json
{"event":"stage_finished","stage_id":"draft","op":"infer","status":"success","model":"mock:bad"}
{"event":"stage_finished","stage_id":"validate","op":"validate_json","status":"invalid","validation_errors":["missing answer"]}
{"event":"stage_finished","stage_id":"repair","op":"repair","status":"success","model":"mock:good"}
{"event":"stage_finished","stage_id":"choose_final","op":"route","status":"success"}
```

With a valid draft, the story changes:

```text
load_prompt   success
draft         success
validate      success
repair        skipped
choose_final  success
```

That skipped status earns its place in the trace. It proves the repair branch was considered and not taken — the graph didn't forget the recovery path; this particular run just didn't need it. When you're reading traces during an incident, the difference between "never declared" and "declared but skipped" is real information.

## Hidden Retries Make Observability Worse

Hidden retry loops are attractive for an honest reason: they keep manifests short. The cost is that they blur the run.

If a wrapper silently retries after a schema miss, what does the supervisor actually see? One model call or three? A final success, or a partial failure that got papered over? A fixed value with no record of the invalid draft that preceded it? You can live without answers to those questions while you're demoing a system. You cannot live without them while you're operating one.

A declared repair stage gives the run a durable shape — `draft -> validate -> repair -> route` — where each node has a stage ID and a duration, model stages report model and token metadata when the backend provides it, validation stages report their errors, and skipped stages appear in the trace without invoking side effects. That's the difference between "the client eventually returned JSON" and "the declared graph repaired an invalid value through `repair` after `validate` rejected it." Only the second sentence is something a supervisor can act on.

## Routes Choose Among Already-Computed Values

`route` is intentionally boring. It doesn't call a model and it doesn't invent a new value — it chooses among stage outputs that already exist in the run. Status routing:

```yaml
- id: choose_final
  op: route
  from: validate
  on_success: validate
  on_invalid: repair
  on_skipped: fallback
```

Field routing:

```yaml
- id: choose_model_output
  op: route
  from: classify
  field: kind
  cases:
    simple: fast_answer
    hard: strong_answer
  default: fast_answer
```

For field routing, the source must be JSON where that's statically checkable — a text source doesn't have a scalar JSON field named `kind`, and `inspect` can reject that shape before the run starts. The boringness is deliberate: routes tend to sit near expensive or user-visible decisions, which is exactly where you want an operation simple enough to verify by reading the inspect report.

```bash
llmff inspect pipeline.yaml --format json
```

## `when` Is A Side-Effect Gate

`when` guards a stage by the status of its parent:

```yaml
- id: repair
  op: repair
  from: validate
  when: invalid
  model: openai:gpt-4.1-mini
  response_format: json
```

The supported conditions are `success`, `invalid`, and `skipped`. When the condition doesn't match, `llmff` marks the stage skipped before any stage-specific work runs — and that ordering is the critical part. A skipped repair stage doesn't call the model. A skipped tool stage doesn't invoke the command. A skipped write stage doesn't touch the file.

That makes `when` more than a convenience flag; it's part of the side-effect boundary. The manifest says when a recovery operation is allowed to happen, and the trace says whether it did.

## Failure Paths Become Testable

Once repair and routing are graph stages, you can test them directly instead of guessing at private control flow. A local deterministic run can force the bad path:

```bash
LLMFF_MOCK_BAD_RESPONSE='{"wrong":true}' \
LLMFF_MOCK_GOOD_RESPONSE='{"answer":"ok"}' \
llmff run examples/json-repair.yaml --trace /tmp/repair.trace.jsonl
```

And the assertions are concrete: `validate` finishes invalid, `repair` finishes success, `choose_final` finishes success, and `answer.json` contains `{"answer":"ok"}`. The valid path is just as testable:

```bash
LLMFF_MOCK_BAD_RESPONSE='{"answer":"already valid"}' \
LLMFF_MOCK_GOOD_RESPONSE='{"answer":"should not be needed"}' \
llmff run examples/json-repair.yaml --trace /tmp/valid.trace.jsonl
```

Here `validate` succeeds, `repair` is skipped, `choose_final` succeeds, and `answer.json` contains `{"answer":"already valid"}`. This is what "explicit failure path" actually buys you — not a slogan, but a set of statuses a test can name and a CI job can check.

## The Caller Still Owns Policy

One thing `llmff` deliberately doesn't decide is whether repair is appropriate for your product. Some invalid outputs should be repaired. Some should be rejected outright. Some should be saved for human review, and some should fail the run because the caller needs a strict contract. The manifest expresses the chosen path for this bounded run; the supervisor or application owns the policy above it.

A strict pipeline can skip repair entirely:

```yaml
graph:
  - id: draft
    op: infer
    from: prompt
    model: openai:gpt-4.1-mini
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

A more forgiving pipeline declares the recovery:

```yaml
graph:
  - id: validate
    op: validate_json
    from: draft
    schema_path: ./answer.schema.json

  - id: repair
    op: repair
    from: validate
    when: invalid
    model: openai:gpt-4.1-mini
    response_format: json

  - id: choose_final
    op: route
    from: validate
    on_success: validate
    on_invalid: repair
```

Both are legitimate product choices. What matters is that the choice is visible before execution, so the person reviewing the manifest knows which one was made.

## Make The Mess Legible

LLM systems don't become easier to operate by pretending failures are rare. They become easier to operate when failure modes have names. `validate_json` names the contract check. `repair` names the semantic recovery call. `route` names the selection among computed values. `when` names the condition under which side effects may happen, and the trace names what actually did.

This is a bounded execution runner doing bounded work — not a planner, not a memory system, not a hidden policy engine. The caller owns why; `llmff` owns what ran. That's why repair and route belong in the manifest: they turn messy model behavior into finite work with visible state.
