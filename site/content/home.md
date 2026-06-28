---
title: "llmff — FFmpeg-shaped pipelines for LLM workflows"
description: "A bounded, inspectable execution runner for typed LLM inference pipelines. Declared graph in, typed artifacts out, process exit code at the boundary."
---

<section class="hero">
  <div class="hero-inner">
    <div>
      <span class="eyebrow">v1.2 · bounded execution runner</span>
      <h1>The <span class="hl">execution layer</span> LLM systems were missing.</h1>
      <p class="lede">llmff is FFmpeg for LLM inference pipelines. Hand it a declared graph; it validates, runs the stages, writes typed artifacts and traces, and exits with a code your supervisor can act on. The caller owns <em>why</em>. llmff owns <em>what ran</em>.</p>
      <div class="cta">
        <a class="btn primary" href="/docs/quickstart.html"><span class="prompt">$</span> get started</a>
        <a class="btn secondary" href="/spec.html">read the spec</a>
        <a class="btn secondary" href="https://github.com/syndicalt/llmff" target="_blank" rel="noreferrer">github ↗</a>
      </div>
    </div>
    <div class="hero-term">
      <div class="bar"><span class="dot r"></span><span class="dot y"></span><span class="dot g"></span><span class="t">~/project — llmff</span></div>
      <div class="body"><div class="ln"><span class="ps1">❯</span> <span class="cmd">llmff</span> <span class="arg">inspect</span> pipeline.yaml <span class="flag">--format</span> json</div>
<div class="ln"><span class="ok">ok</span> <span class="dim"># graph valid · 6 stages · backends resolved</span></div>
<div class="ln">&nbsp;</div>
<div class="ln"><span class="ps1">❯</span> <span class="cmd">llmff</span> <span class="arg">run</span> pipeline.yaml <span class="flag">--trace</span> run.jsonl</div>
<div class="ln"><span class="key">[load]</span> ok  <span class="key">[retrieve]</span> ok  <span class="key">[infer]</span> ok</div>
<div class="ln"><span class="key">[validate_json]</span> <span class="err">fail</span> → <span class="key">[repair]</span> <span class="ok">ok</span>  <span class="key">[write]</span> ok</div>
<div class="ln"><span class="out">run complete · 1 stage repaired · usage=1280 tokens</span></div>
<div class="ln">&nbsp;</div>
<div class="ln"><span class="ps1">❯</span> <span class="cmd">echo</span> $?</div>
<div class="ln"><span class="ok">0</span><span class="cursor"></span></div>
      </div>
    </div>
  </div>
</section>

<section class="section center">
  <h2><span class="prompt">$</span> llmff --help</h2>
  <p class="sub">A small, sharp set of primitives. Every one is declared in the manifest, inspectable before it runs, and recorded after it exits.</p>
  <div class="grid">
    <div class="card"><div class="ic">// graph</div><h3>Typed pipeline graphs</h3><p>Reproducible YAML manifests or compact inline graphs. Ordered, dependency-checked stages with conservative type compatibility.</p><a class="more" href="/docs/pipeline-library.html">pipeline library</a></div>
    <div class="card"><div class="ic">// preflight</div><h3>Inspect before you run</h3><p><code>inspect --format json</code> turns a manifest into a machine-readable contract — no model calls, no payloads. Trust starts before execution.</p><a class="more" href="/blog/05-inspect-before-you-run.html">why inspect</a></div>
    <div class="card"><div class="ic">// backends</div><h3>Backends &amp; providers</h3><p>OpenAI-compatible and Ollama adapters, deterministic offline mocks, and plugin command backends — registered explicitly on the command line.</p><a class="more" href="/docs/providers/support-tiers.html">providers</a></div>
    <div class="card"><div class="ic">// json</div><h3>Validation &amp; repair</h3><p>Validate stage output against a JSON Schema, run a bounded repair pass on failure, and <code>route</code> to a fallback — all as declared stages.</p><a class="more" href="/blog/04-repair-route-failure-paths.html">failure paths</a></div>
    <div class="card"><div class="ic">// control</div><h3>Bounded loops &amp; maps</h3><p><code>loop</code> repeats a subgraph to a hard <code>max_iterations</code>; <code>map</code> fans a subgraph over an array with <code>max_items</code>. Repetition as a primitive, never an autonomous orchestrator.</p><a class="more" href="/blog/08-bounded-loops.html">bounded loops</a></div>
    <div class="card"><div class="ic">// transform</div><h3>Deterministic transforms</h3><p><code>extract</code>, <code>predicate</code>, <code>score</code>, <code>select</code>, and <code>accumulate</code> shape JSON between model calls and carry state across iterations. No network, no hidden expressions.</p><a class="more" href="/blog/03-typed-values-validation.html">typed values</a></div>
    <div class="card"><div class="ic">// agents</div><h3>Declared multi-agent topology</h3><p>Name reusable <code>agents:</code> roles and reference them from <code>infer</code>/<code>repair</code> stages. Pure inspect-time sugar that stays a bounded DAG.</p><a class="more" href="/docs/agent-workflows.html">agent workflows</a></div>
    <div class="card"><div class="ic">// observe</div><h3>Traces, events &amp; resume</h3><p>JSONL traces and lifecycle events, checkpoint/resume, batch mode, and stable exit codes with additive failure kinds. Local-first observability.</p><a class="more" href="/docs/observability.html">observability</a></div>
  </div>
</section>

<section class="band">
  <div class="split">
    <div class="prose">
      <h2><span class="prompt">#</span> Structured graphs, not another loop</h2>
      <p>The dominant way to build LLM agents is the <em>agent loop</em>: one model reads an ever-growing context window and decides what to do next. That paradigm has three structural weaknesses — implicit dependencies between steps, unbounded recovery loops, and a mutable execution history that is hard to debug.</p>
      <p>The fix that is now coming into vogue is to lift control flow out of implicit context and into an explicit, static DAG — a plan you can inspect before it runs and audit after it exits. That is exactly what llmff is: the bounded execution layer underneath whatever owns intent.</p>
      <p class="cite">Hu Wei, <em>From Agent Loops to Structured Graphs: A Scheduler-Theoretic Framework for LLM Agent Execution</em>, arXiv:2604.11378 (2026) — places agent loops and graph execution engines on one semantic continuum.</p>
      <div class="cta" style="margin-top:22px"><a class="btn secondary" href="/research.html"><span class="prompt">$</span> read the research</a></div>
    </div>
    <div class="cmd-card">
      <div class="bar"><span class="dot r"></span><span class="dot y"></span><span class="dot g"></span><span class="t">agent-loop.log vs llmff.dag</span></div>
      <div class="body"><div class="ln"><span class="err">#</span> <span class="dim">agent loop — single ready unit, opaque next step</span></div>
<div class="ln"><span class="dim">while not done:</span></div>
<div class="ln"><span class="dim">  step = llm(context)   </span><span class="err"># implicit deps</span></div>
<div class="ln"><span class="dim">  context += run(step)  </span><span class="err"># mutable history</span></div>
<div class="ln"><span class="dim">  # may retry forever   </span><span class="err"># unbounded recovery</span></div>
<div class="ln">&nbsp;</div>
<div class="ln"><span class="ok">#</span> <span class="dim">llmff — explicit static DAG</span></div>
<div class="ln"><span class="key">graph:</span></div>
<div class="ln">  - id: draft   <span class="key">op:</span> <span class="arg">infer</span></div>
<div class="ln">  - id: check   <span class="key">op:</span> <span class="arg">validate_json</span></div>
<div class="ln">  - id: fix     <span class="key">op:</span> <span class="arg">repair</span>   <span class="flag">when:</span> check.failed</div>
<div class="ln">  - id: out     <span class="key">op:</span> <span class="arg">write</span>    <span class="ok"># bounded, inspectable</span></div>
      </div>
    </div>
  </div>
</section>

<section class="section">
  <h2><span class="prompt">$</span> llmff stages list</h2>
  <p class="sub">Nineteen built-in stage operations. Compose them into a graph; <code>inspect</code> proves it before a single token is spent.</p>
  <table class="stage-table">
    <thead><tr><th>op</th><th>kind</th><th>what it does</th></tr></thead>
    <tbody>
      <tr><td class="op">load</td><td class="k">input</td><td>Read a declared input (text/JSON/stdin) into the graph.</td></tr>
      <tr><td class="op">template</td><td class="k">prompt</td><td>Render a file-backed prompt template with JSON fields.</td></tr>
      <tr><td class="op">system</td><td class="k">prompt</td><td>Attach a system prompt / chat messages.</td></tr>
      <tr><td class="op">retrieve</td><td class="k">retrieval</td><td>Local document retrieval with a selectable strategy.</td></tr>
      <tr><td class="op">rerank</td><td class="k">retrieval</td><td>Re-order retrieved documents by relevance.</td></tr>
      <tr><td class="op">infer</td><td class="k">model</td><td>Call a backend model; streams deltas when supported.</td></tr>
      <tr><td class="op">validate_json</td><td class="k">validation</td><td>Validate output against an inline schema or <code>schema_path</code>.</td></tr>
      <tr><td class="op">repair</td><td class="k">model</td><td>Bounded re-inference to fix invalid JSON.</td></tr>
      <tr><td class="op">route</td><td class="k">control</td><td>Select a downstream value by status or condition.</td></tr>
      <tr><td class="op">extract</td><td class="k">transform</td><td>Pull the value at a JSON dot-path; fail if missing.</td></tr>
      <tr><td class="op">predicate</td><td class="k">transform</td><td>Evaluate a typed condition → <code>{passed, …}</code>.</td></tr>
      <tr><td class="op">score</td><td class="k">transform</td><td>Read a numeric score, optionally bounded.</td></tr>
      <tr><td class="op">select</td><td class="k">transform</td><td>Choose one entry from an array of candidates.</td></tr>
      <tr><td class="op">accumulate</td><td class="k">transform</td><td>Carry state across loop iterations.</td></tr>
      <tr><td class="op">loop</td><td class="k">control</td><td>Repeat a body subgraph to a hard bound.</td></tr>
      <tr><td class="op">map</td><td class="k">control</td><td>Fan a body subgraph over a JSON array.</td></tr>
      <tr><td class="op">tool</td><td class="k">tool</td><td>Declared subprocess or HTTP tool call.</td></tr>
      <tr><td class="op">cache</td><td class="k">storage</td><td>Persistent cache for a stage value.</td></tr>
      <tr><td class="op">write</td><td class="k">output</td><td>Write a value to a declared artifact path.</td></tr>
    </tbody>
  </table>
  <div class="cta" style="margin-top:26px"><a class="btn primary" href="/docs/quickstart.html"><span class="prompt">$</span> llmff quickstart</a><a class="btn secondary" href="/examples/README.html">browse examples</a></div>
</section>
