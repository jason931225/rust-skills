# async-yield-cpu

> Yield between chunks of long CPU work so other tasks can run

## Why It Matters

An `async fn` that decompresses a whole archive without awaiting never gives the runtime a chance to poll anything else on that worker. I/O-bound loops already yield at each `.await`; CPU-bound loops do not. The Microsoft Pragmatic Rust Guidelines ask for an explicit `yield_now().await` every few tens of microseconds of compute so one request cannot starve the rest of the process.

## Bad

```rust
fn decompress(item: &[u8]) -> Vec<u8> {
    item.to_vec()
}

async fn process_items(items: &[&[u8]]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for item in items {
        // No await in the loop: this future occupies the worker until every
        // item is done.
        out.push(decompress(item));
    }
    out
}
```

## Good

```rust
fn decompress(item: &[u8]) -> Vec<u8> {
    item.to_vec()
}

async fn process_items(items: &[&[u8]]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for item in items {
        out.push(decompress(item));
        tokio::task::yield_now().await;
    }
    out
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let blob: &[u8] = b"payload";
    let items = [blob, blob];
    let out = process_items(&items).await;
    assert_eq!(out.len(), 2);
}
```

## See Also

- [async-spawn-blocking](async-spawn-blocking.md) - move multi-millisecond CPU work off the runtime entirely
- [async-join-parallel](async-join-parallel.md) - split independent chunks into concurrent tasks after you can yield
- [async-no-lock-await](async-no-lock-await.md) - do not hold a lock across the yield point you just added
