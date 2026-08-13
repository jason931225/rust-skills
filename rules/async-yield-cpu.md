# async-yield-cpu

> Yield between chunks of long CPU work so other tasks can run

## Why It Matters

An `async fn` that inflates a whole archive without awaiting never gives the runtime a chance to poll anything else on that worker. I/O-bound loops already yield at each `.await`; CPU-bound loops do not. Following Microsoft Pragmatic Rust Guidelines (M-YIELD-POINTS), insert an explicit `yield_now().await` every few tens of microseconds of compute so one request cannot starve the rest of the process.

## Bad

```rust
fn inflate_chunk(item: &[u8]) -> Vec<u8> {
    item.to_vec()
}

async fn inflate_batch(items: &[&[u8]]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for item in items {
        // No await in the loop: this future occupies the worker until every
        // item is done.
        out.push(inflate_chunk(item));
    }
    out
}
```

## Unpredictable Work

A fixed item count is only a proxy for time. When item cost or batch length
varies widely, use the cooperative-budget signal exposed by the hosting
runtime and yield when its budget is exhausted. Tokio, for example, exposes
`tokio::task::coop::has_budget_remaining()`. That API is Tokio-specific; a
runtime-neutral library should put the policy behind its runtime adapter
instead of leaking Tokio into its public API.

For multi-millisecond CPU work, budget checks are not enough—move the work to
`spawn_blocking` or a dedicated compute pool.

## Good

```rust
fn inflate_chunk(item: &[u8]) -> Vec<u8> {
    item.to_vec()
}

async fn inflate_batch(items: &[&[u8]]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for item in items {
        out.push(inflate_chunk(item));
        // Yield so other tasks on this worker can run between chunks.
        tokio::task::yield_now().await;
    }
    out
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let blob: &[u8] = b"payload";
    let items = [blob, blob];
    let out = inflate_batch(&items).await;
    assert_eq!(out.len(), 2);
}
```

## See Also

- [async-spawn-blocking](async-spawn-blocking.md) - move multi-millisecond CPU work off the runtime entirely
- [async-join-parallel](async-join-parallel.md) - split independent chunks into concurrent tasks after you can yield
- [async-no-lock-await](async-no-lock-await.md) - do not hold a lock across the yield point you just added
