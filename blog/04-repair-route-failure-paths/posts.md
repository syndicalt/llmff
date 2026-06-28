# Companion Posts

1. Retries are for flaky transport. Repair is for bad semantics. Treating them as the same operation hides the real failure mode.

2. A route stage is a production incident waiting to not happen: it makes the recovery choice visible before the run and auditable after it.

3. `when: invalid` is a side-effect gate. If validation succeeds, repair is skipped before the model call happens.

4. If a failure path matters, it belongs in the manifest.

5. Hidden retry loops can return a clean final value while erasing the invalid draft that caused the recovery. Declared repair keeps the evidence in the run.

6. `route` should be boring: choose among already-computed stage outputs by status or JSON field. No new value, no hidden policy.
