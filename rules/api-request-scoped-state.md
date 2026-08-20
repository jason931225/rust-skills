# api-request-scoped-state

> Build shared application state outside the per-worker factory and clone a handle into each worker; read request-scoped values from the framework's typed extension map, not from handler parameters alone

## Why It Matters

An async web framework typically builds one application instance per worker
thread (or per connection), by calling a factory closure once for each. A
value constructed *inside* that closure — a counter, a cache, a database pool
built fresh each time — is worker-local: a request handled by worker A never
sees a mutation made while handling a request on worker B, even though both
ran the identical-looking closure. This reads as flaky or impossible behavior
("the counter reset itself") until the factory/worker split is visible. A
second, related surface is request-scoped values that are not function
parameters at all: an authentication middleware's output, a request id, or a
value an extractor-rejection handler needs is commonly threaded through a
typed, `TypeId`-keyed extension map rather than an ordinary argument, and a
handler or error hook that expects one without it being registered fails —
often only on the error path, which is the path least likely to be exercised
in casual testing.

## Worker And Request State Rules

- Construct any value that must be visible to every worker — a shared cache,
  a counter, a connection pool — once, outside the per-worker factory
  closure, and move a cheap shared handle (`Arc<T>`, a pool handle) into each
  worker's closure. Never rely on identical initialization code producing
  shared state; identical code run once per worker produces worker-local
  state.
- Verify shared state with a test that forces requests across more than one
  worker (or explicitly configure a single worker in the test, and document
  that the multi-worker case is untested if so).
- Treat the framework's typed extension/state map as its own storage: values
  placed there by middleware are retrieved by type in downstream handlers and
  error hooks, and a missing registration is a runtime failure at the
  retrieval site, not a compile error.
- Error hooks and other framework-invoked callbacks that are not ordinary
  handlers often cannot take the same extractors a handler can; anything they
  need has to arrive through the typed extension map (or another
  framework-specific side channel), set up before the callback can run.
- Test the failure path deliberately: trigger whatever invokes an error hook
  or middleware-fed handler in an environment where the expected extension
  value was never registered, and confirm the failure is a clear diagnostic,
  not a panic with no context.

## Bad

```rust
// A counter built inside the per-worker factory. Each worker gets its own
// independent counter starting at zero; a client that hits three different
// workers in three requests sees three separate "first request" counts
// instead of one shared count of three.
fn app_factory() -> App {
    let request_count = std::cell::Cell::new(0);
    App::new().data(request_count)
}
```

## Good

```rust
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Built once, before any worker factory runs.
struct SharedState {
    request_count: AtomicU64,
}

/// The factory receives a clone of the shared handle, not a fresh value —
/// every worker's closure captures the *same* underlying counter.
fn app_factory(shared: Arc<SharedState>) -> impl Fn() -> Arc<SharedState> {
    move || Arc::clone(&shared)
}

fn handle_request(state: &SharedState) -> u64 {
    state.request_count.fetch_add(1, Ordering::Relaxed) + 1
}

fn main() {
    let shared = Arc::new(SharedState { request_count: AtomicU64::new(0) });
    let worker_a = app_factory(Arc::clone(&shared))();
    let worker_b = app_factory(Arc::clone(&shared))();

    // A request handled by "worker A" and one handled by "worker B" both
    // observe and advance the same counter, because both hold a clone of
    // the same Arc rather than two independently constructed counters.
    assert_eq!(handle_request(&worker_a), 1);
    assert_eq!(handle_request(&worker_b), 2);
    assert_eq!(handle_request(&worker_a), 3);
}
```

## Sharing Failures To Test

- state mutated while handling a request on one simulated worker is visible
  when handling a request on a different simulated worker;
- two independently constructed values built inside separate factory calls
  are proven to be distinct allocations (the bug this rule prevents), so the
  fix's `Arc::clone` sharing is the only thing that changes that;
- a callback that reads a required value from the typed extension map fails
  with a clear error, not a panic with no message, when that value was never
  registered;
- the request path that populates the extension map runs before any path
  that reads from it, in both the success and the error-hook cases.

## See Also

- [api-service-clone](api-service-clone.md) - the cheap-`Clone`-handle shape shared application state should take
- [own-arc-shared](own-arc-shared.md) - `Arc` as the mechanism that makes one value visible to every worker
- [api-extract-or-reject](api-extract-or-reject.md) - the extractor pipeline this rule's error-hook case sits downstream of
- [obs-request-correlation](obs-request-correlation.md) - a common example of a value carried through request-scoped state
- [proj-avoid-statics](proj-avoid-statics.md) - why shared state is built once and passed down, not reached for as a global
