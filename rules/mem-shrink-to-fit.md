# mem-shrink-to-fit

> Call `shrink_to_fit` on long-lived collections built without an exact capacity

## Why It Matters

`Vec` and `String` grow by doubling. A buffer that settled at 1,025 elements can keep a 2,048-element allocation for the rest of the process. Per Microsoft Pragmatic Rust Guidelines (M-SHRINK-TO-FIT), reserve `shrink_to_fit` for collections that will live a long time after a growth loop, not for short-lived scratch buffers where the extra copy would cost more than the slack.

## Bad

```rust
pub fn collect_labels(items: &[&str]) -> Vec<String> {
    let mut labels = Vec::new();
    for item in items {
        labels.push((*item).to_string());
    }
    // `labels` may hold nearly 2× the bytes it needs for the rest of the run.
    labels
}
```

## Good

```rust
pub fn collect_labels(items: &[&str]) -> Vec<String> {
    let mut labels = Vec::with_capacity(items.len());
    for item in items {
        labels.push((*item).to_string());
    }
    labels.shrink_to_fit(); // drop spare capacity on a long-lived buffer
    labels
}

fn main() {
    let labels = collect_labels(&["alpha", "beta"]);
    assert_eq!(labels.capacity(), 2);
}
```

## See Also

- [mem-with-capacity](mem-with-capacity.md) - reserve first so you rarely need to shrink
- [mem-boxed-slice](mem-boxed-slice.md) - `into_boxed_slice` already shrinks and drops the spare capacity field
- [mem-reuse-collections](mem-reuse-collections.md) - do not shrink a buffer you are about to refill
