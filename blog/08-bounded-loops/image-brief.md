# Image Brief

Create a clean technical diagram for Article 8.

Concept: show a single `op: loop` stage expanded into three finite iterations.

- Left side: outer graph node labeled `refine_loop`.
- Right side: three columns labeled iteration 1, iteration 2, iteration 3.
- Each column contains the same body DAG: `draft -> validate_json -> predicate/check`.
- A break-condition gate should sit after each iteration with a label like `break_on: stage_success(check)`.
- Add a small trace label under one body event: `loop_id=refine_loop`, `loop_iteration=2`, `loop_stage_id=draft`.
- Include a top label: `max_iterations: 3` and a small note: "break can stop earlier, never later".

Visual direction: white or light background, precise stage boxes, thin arrows, restrained color. Avoid visual language that suggests autonomy, agents, or open-ended planning. The image should teach bounded unrolling and trace context.
