# proj-stateless-process

> Keep durable application state outside individual service processes

## Why It Matters

Replicas are disposable: they restart, move hosts, and scale independently.
State written only to a local filesystem or in-memory registry disappears with
one instance and is invisible to the others. Persist durable domain state in a
shared service with an explicit consistency contract; keep process-local state
limited to caches and bounded work that can be reconstructed.
This rule targets horizontally scaled, interchangeable application replicas;
databases, brokers, and stateful stream processors have a separate durable
identity and migration contract.

## Bad

- Keep authoritative uploads, sessions, or jobs only in process memory or on a
  replica's local filesystem.
- Require replacement replicas to recover unpublished files from the process
  they replace.

## Good

- Externalize durable state to a database, object store, or log appropriate to
  its access and consistency needs.
- Treat local caches as disposable: bound them, invalidate or version them, and
  recover from a cold start.
- Store uploads and generated artifacts durably before returning a success
  that promises persistence. Use deterministic ownership keys, an orphan
  retention window, and a reconciler because the object write can commit
  before its metadata reference.
- Sessions, idempotency records, queue tasks, leader state, rate limits,
  login-attempt counters, and one-time-use markers such as nonces, JTIs, OTPs,
  CSRF tokens, and password-reset tokens require shared storage or a platform
  primitive. A per-process counter or replay cache fails open across replicas.
- Startup does not depend on a previous process's local files.
- State the shared-store failure policy: retry with bounded backoff, report
  unready when the service cannot honor its contract, and never silently fall
  back to process-local authority or enter a fleet-wide crash loop.
- Emit shared-store availability, saturation, cache-staleness, and
  reconciliation-backlog signals with bounded cardinality.
- Tests run multiple replicas or restart the process to prove state survives
  and remains coherent, including concurrent rate-limit and replay attempts.

## See Also

- [proj-schema-migrations](proj-schema-migrations.md) - evolve shared database state
- [api-session-security](api-session-security.md) - keep authoritative sessions server-side and shared
- [api-idempotency-key](api-idempotency-key.md) - retry records survive process loss
- [async-durable-worker](async-durable-worker.md) - in-memory spawned work is not durable
- [proj-continuous-delivery](proj-continuous-delivery.md) - replace replicas without state loss
