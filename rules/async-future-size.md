# async-future-size

> Keep hot `async fn` state machines small: do not capture large values across `.await`, and box the heavy branch

## Why It Matters

Locals that live across `.await`, plus by-value arguments, become fields of the future. A 4 KiB buffer held over one I/O point is copied onto the worker stack and then into the task allocation on every poll setup. Following Microsoft Pragmatic Rust Guidelines (M-ASYNC-STACK-SIZE), hot entry points should track `size_of_val(&fut)` (or `static_assertions` / a unit test) and move setup *outside* `async`. `clippy::large_futures` (allowlisted in `clippy::nursery`, enable it for the crate) is the mechanical tripwire; `Box::pin` or `futures::future::Either` keeps the rare large path off the common type.

## Bad

```rust
pub struct Huge([u8; 4096]);

pub async fn send(payload: Huge) -> usize {
    let scratch = [0u8; 4096];
    async { 1 }.await;
    payload.0[0] as usize + scratch[0] as usize
}
```

## Good

```rust
pub struct Huge([u8; 32]);

pub fn send(payload: Huge) -> impl Future<Output = usize> {
    let first = payload.0[0];
    async move {
        async { 1 }.await;
        first as usize
    }
}

#[test]
fn send_future_stays_small() {
    let fut = send(Huge([0; 32]));
    assert!(std::mem::size_of_val(&fut) < 256);
}

fn main() {
    let _ = send(Huge([7; 32]));
}
```

## See Also

- [mem-box-large-variant](mem-box-large-variant.md) - box the large enum arm the same way you box a large future branch
- [mem-assert-type-size](mem-assert-type-size.md) - the same size tripwire for ordinary structs
- [async-spawn-blocking](async-spawn-blocking.md) - multi-kilobyte CPU work does not belong in the future at all
