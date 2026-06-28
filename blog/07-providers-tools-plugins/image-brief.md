# Image Brief

Create a clean technical boundary diagram for Article 7.

Concept: three layers with explicit arrows.

- Top: "Supervisor / application" with labels "policy", "tool catalog", "provider choice", "approval".
- Middle: "`llmff` execution runner" with labels "declared graph", "tool stage", "backend alias", "typed outputs", "trace metadata".
- Bottom: four boxes: "OpenAI-compatible backend", "Ollama backend", "command tool", "HTTP / plugin transport".

Visual direction: white or very light background, thin neutral lines, restrained accent colors. It should feel like an architecture diagram from a systems design doc, not a marketing graphic. Avoid mascots, clouds, glowing nodes, and decorative gradients. The key visual point is that policy sits above `llmff`, while transports sit below it.
