# Companion Posts

1. `llmff` can run a tool. It should not decide which tools your product is allowed to use.

   Tool catalog policy belongs above the runner. The manifest declares the call; `llmff` executes it and emits traceable stage state.

2. Provider aliases keep manifests portable without hiding the backend.

   `model: openai:gpt-4.1-mini` means `openai` is the registered backend alias and `gpt-4.1-mini` is the provider model ID.

3. A command tool is an explicit stage with stdin/stdout semantics.

   Serialized parent value in. Captured stdout out. Then validate the output before feeding it back into the graph.

4. Plugin manifests are trust-boundary documents.

   Capability kind, name, version, and entrypoint are not metadata fluff. They are what a supervisor can inspect before allowing a plugin directory into a run.

5. Tool loops should be typed at both edges.

   Validate the model-produced request before the tool call. Validate the tool result before accumulation. Hidden tool state is where debugging gets expensive.
