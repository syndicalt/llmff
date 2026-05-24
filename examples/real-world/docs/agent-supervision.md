Agent supervisors should call llmff as a bounded subprocess. The supervisor
owns planning, retry policy, memory, and user approval. llmff owns one pipeline
run with explicit inputs, trace artifacts, lifecycle events, checkpoint state,
and a process exit code.

Use inspect before running when the supervisor needs a machine-readable
execution contract. Keep payload outputs in declared artifact paths. Keep
events and traces as metadata streams. Treat the exit code as final authority,
then inspect failure_kind for retry, repair, or escalation decisions.
