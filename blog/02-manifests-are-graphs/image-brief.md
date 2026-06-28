# Image Brief

Create a clean DAG concept image for the header.

Concept: manifest stage IDs as the shared handles across graph execution, trace, and outputs.

Structure:

- Main diagram: left-to-right DAG with nodes:
  - `load_prompt`
  - `build_prompt`
  - `draft`
  - `validate_answer`
  - `write_answer`
- Each node should show the operation in smaller text, such as `op: load` or `op: infer`.
- Add arrows for dependencies.
- Add a side panel with three rows:
  - `manifest`
  - `trace`
  - `output`
- Draw thin connector lines from stage IDs in the graph to the same IDs in the side panel.
- Use neutral colors and a clear educational style.

Avoid:

- Decorative abstract network art.
- Dense unreadable labels.
- Marketing language.
- Robot or chat bubble imagery.
