---
title: "Research & Foundations"
description: "Why structured graphs are replacing the agent loop — and how llmff implements the bounded execution layer that line of research points to."
---

<section class="section narrow">
<div class="page-head"><div class="kicker">$ llmff research --foundations</div>
<h1>Research &amp; Foundations</h1>
<p>llmff is an opinionated bet: the missing abstraction in LLM systems is not another agent loop, it is a bounded, inspectable execution layer. That bet now has a growing theoretical backbone.</p></div>

<article class="doc-content">

## The shift: agent loops → structured graphs

For two years the default way to build an LLM agent has been the **agent loop**: a single model reads an ever-growing context window and decides what to do next, one step at a time. It is easy to start and hard to operate. The loop-engineering wave that followed — ever more elaborate prompting, memory, and retry scaffolding around that single loop — papered over three structural weaknesses rather than removing them.

A recent position paper makes the weaknesses precise:

> The dominant paradigm for building LLM based agents is the Agent Loop … This paradigm has three structural weaknesses: implicit dependencies between steps, unbounded recovery loops, and mutable execution history that complicates debugging. We characterize the Agent Loop as a single ready unit scheduler: at any moment, at most one executable unit is active, and the choice of which unit to activate comes from opaque LLM inference rather than an inspectable policy.
>
> — Hu Wei, *From Agent Loops to Structured Graphs* (arXiv:[2604.11378](https://arxiv.org/abs/2604.11378))

The paper's proposal, **SGH (Structured Graph Harness)**, "lifts control flow from implicit context into an explicit static DAG" and makes three commitments: execution plans are **immutable within a plan version**; **planning, execution, and recovery are separated** into three layers; and **recovery follows a strict escalation protocol**. Those choices deliberately trade some expressiveness for controllability, verifiability, and implementability.

That continuum — from opaque single-ready-unit loops to inspectable static graphs — is exactly the design space llmff occupies. llmff does not try to be the planner. It is the execution substrate a planner (or a CI job, a queue worker, a human) hands a declared graph to.

## How llmff maps to the Structured Graph Harness

llmff predates this paper but lands on the same commitments from the implementation side. The mapping is direct:

| SGH commitment | llmff mechanism |
| --- | --- |
| Explicit static DAG, not implicit context | YAML manifests / inline graphs with ordered, dependency-checked stages |
| Plans immutable within a plan version | Pinned manifests + `inspect --format json` preflight contract; checkpoints bound to a manifest hash |
| Planning ⟂ execution ⟂ recovery | Caller owns planning; llmff executes the declared graph; `repair` / `route` / retries are *declared* recovery stages |
| Strict escalation protocol | Bounded `repair` → `route` fallback → non-zero **exit code** + additive **failure kind** for the supervisor |
| Node state machine with termination guarantees | Typed stage statuses; bounded `loop` (`max_iterations`) and `map` (`max_items`); deterministic transforms |
| Inspectable policy instead of opaque next-step inference | The graph *is* the policy — readable before the run, recorded in traces after it |

The single most important line both share: the system that runs the work should **not** decide why the work exists. In SGH terms, llmff is the harness, not the planner. In llmff's own terms, the caller owns *why*; llmff owns *what ran*.

## What stays above the boundary

The paper is careful that lifting control flow into a DAG trades expressiveness for control. llmff makes the same trade on purpose. It deliberately does **not** grow autonomous planning, persistent memory, task scheduling, or dynamic agent-to-agent dispatch where a model picks an undeclared successor. Declared `agents:` roles name a topology; they never decide which role runs next at runtime. Those orchestration concerns belong to the host — see the [Specification](/spec.html) and the [v1 Contract](/docs/v1-contract.html) for the exact boundary.

## llmff's own design corpus

There is no peer-reviewed paper authored *for* llmff yet. Its design rationale lives in primary documents and a long-form essay series, written as the project was built:

- **[Specification](/spec.html)** — the canonical product boundary and roadmap.
- **[v1 Contract](/docs/v1-contract.html)** — the compatibility surface: CLI, schemas, events, traces, exit codes.
- **The blog series** — ten essays working through the execution layer, manifests-as-graphs, typed values, failure paths, inspection, observability, bounded loops, and why the boundary is the product. Start with [The Execution Layer LLM Systems Were Missing](/blog/01-execution-layer.html).

If you are evaluating llmff against the structured-graph literature, those three are the most direct statements of intent.

## Cite the foundational paper

```bibtex
@article{huwei2026agentloops,
  title   = {From Agent Loops to Structured Graphs: A
             Scheduler-Theoretic Framework for LLM Agent Execution},
  author  = {Hu, Wei},
  journal = {arXiv preprint arXiv:2604.11378},
  year    = {2026},
  note    = {Position paper; cs.AI, eess.SY},
  url     = {https://arxiv.org/abs/2604.11378}
}
```

</article>
</section>
