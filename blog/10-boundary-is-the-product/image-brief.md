# Image Brief

Create a clean technical architecture image for Article 10, "The Boundary Is The Product."

Concept: three-layer boundary stack.

Top layer: "Supervisor / agent host" with small labels: planning, memory, policy, approval, scheduling, retries, retention. This layer owns "why."

Middle layer: highlighted box labeled "`llmff`: bounded manifest execution." Inside the box: manifest, typed stages, inspect report, events, trace, checkpoint, result, exit code. Draw a clear subprocess boundary around this middle layer. This layer owns "what ran."

Bottom layer: "Execution dependencies" with labels: model providers, command tools, HTTP tools, plugins, files, schemas. This layer owns "how calls execute."

Include a simple arrow sequence beside the stack: `inspect -> run -> preserve exit code -> store artifacts -> decide next step`.

Style: restrained systems diagram, white or light background, precise labels, thin lines. No mascot, no decorative art, no hype language. The image should clarify ownership boundaries, not sell the product.
