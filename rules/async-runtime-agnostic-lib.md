# async-runtime-agnostic-lib

> A library takes futures and returns futures; the binary that owns `main` picks the runtime

## Why It Matters

A library that calls `tokio::spawn`, sleeps on a runtime timer, or opens a
runtime socket has chosen an executor for every downstream binary, and it does
so invisibly — the dependency is not in the signature, so the caller discovers
it as a panic saying there is no reactor running. Applications legitimately own
that decision, and two runtimes in one process is a configuration nobody
intends. The same reasoning already applies to installing a global tracing
subscriber; the runtime deserves the same discipline.

## Bad

```rust
// In a library crate
pub async fn fetch_all(urls: Vec<Url>) -> Vec<Response> {
    let handles: Vec<_> = urls.into_iter()
        .map(|url| tokio::spawn(fetch(url)))   // chooses the caller's runtime
        .collect();
    // ...and panics under any other executor
}
```

## Good

```rust
use std::future::Future;

/// Takes the work as futures and composes them. No spawn, no timer, no socket:
/// whatever drives the returned future decides the runtime.
pub async fn fetch_all<F>(requests: Vec<F>) -> Vec<F::Output>
where
    F: Future,
{
    let mut results = Vec::with_capacity(requests.len());
    for request in requests {
        results.push(request.await);
    }
    results
}

/// Concurrency the caller controls: hand the library a spawner rather than
/// reaching for one.
pub async fn fetch_with<S, F>(requests: Vec<F>, spawn: S) -> Vec<F::Output>
where
    F: Future,
    S: Fn(F) -> F,
{
    let mut results = Vec::with_capacity(requests.len());
    for request in requests {
        results.push(spawn(request).await);
    }
    results
}

fn main() {
    let outputs = futures::executor::block_on(fetch_all(vec![
        std::future::ready(1),
        std::future::ready(2),
    ]));
    assert_eq!(outputs, vec![1, 2]);

    // The same library code runs under a different executor without changes.
    let outputs = futures::executor::block_on(fetch_with(
        vec![std::future::ready("a")],
        |future| future,
    ));
    assert_eq!(outputs, vec!["a"]);
}
```

## Keeping Runtime Dependencies Out

- Depend on `std::future::Future` and, where a stream is needed, on the
  `futures` traits — not on a runtime crate.
- Where a library genuinely needs to spawn, take a spawner or a handle as a
  parameter so the choice stays with the caller.
- Gate any runtime integration behind an optional, non-default feature, and
  keep the default build runtime-free.
- Timers and I/O are the usual leaks: `tokio::time::sleep` and
  `tokio::net::TcpStream` bind the caller as firmly as `spawn` does.
- A test that only ever runs under `#[tokio::test]` will not catch this; build
  the crate with default features and no runtime dependency in CI.

## See Also

- [obs-library-facade](obs-library-facade.md) - the same ownership rule for the tracing subscriber
- [async-tokio-runtime](async-tokio-runtime.md) - the application side of the decision
- [proj-feature-additive](proj-feature-additive.md) - gating an optional runtime integration
- [api-std-types-boundary](api-std-types-boundary.md) - keeping foreign types out of the public surface
