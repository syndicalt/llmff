# Image Brief

Concept image for Article 4: a simple failure-path flowchart for JSON repair.

Show this path:

`draft` -> `validate_json`

From `validate_json`, split by status:

- `success` routes directly into `choose_final`.
- `invalid` goes to `repair`, then into `choose_final`.

Add a small `when: invalid` label on the repair edge or repair node. Show `repair` as skipped on the success path. End at `answer.json`.

The image should emphasize explicit graph structure: stage IDs, status labels, and the final route. Keep it practical and diagrammatic, with no marketing copy beyond the labels needed to understand the flow.
