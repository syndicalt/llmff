# Companion Posts

1. Text is the worst possible interface between two LLM workflow stages. It looks flexible until every downstream stage has to rediscover the contract.

2. `response_format: json` is a model hint. `validate_json` is the workflow contract. Confusing those two is how structured-output bugs become runtime folklore.

3. Invalid JSON is not always a process failure. Sometimes it is a typed state the graph should route, repair, trace, or preserve.

4. Validation is where an LLM pipeline becomes an interface: stage ID, schema, status, trace event, artifact.

5. The useful distinction is `Result<StageStatus, StageError>`. Missing schema file is execution failure. Missing `answer` field is semantic invalidity.

6. Ambiguity compounds until the graph names it.
