# perf-iter-lazy

> Keep iterators lazy, collect only when needed

## Why It Matters

Rust iterators are lazy—they compute values on demand. This enables single-pass processing, avoids intermediate allocations, and allows short-circuiting. Calling `.collect()` too early forces evaluation and allocates unnecessarily.

## Bad

```rust
// Collects intermediate results unnecessarily
fn process(data: Vec<i32>) -> Vec<i32> {
    let filtered: Vec<_> = data.into_iter()
        .filter(|x| *x > 0)
        .collect();  // Unnecessary allocation
    
    let mapped: Vec<_> = filtered.into_iter()
        .map(|x| x * 2)
        .collect();  // Another unnecessary allocation
    
    mapped.into_iter()
        .take(10)
        .collect()
}

// Collects before checking existence
fn has_positive(data: &[i32]) -> bool {
    let positives: Vec<_> = data.iter()
        .filter(|&&x| x > 0)
        .collect();  // Allocates entire filtered result
    
    !positives.is_empty()
}
```

## Good

```rust
// Single chain, single collect
fn process(data: Vec<i32>) -> Vec<i32> {
    data.into_iter()
        .filter(|x| *x > 0)
        .map(|x| x * 2)
        .take(10)
        .collect()
}

// Short-circuits on first match
fn has_positive(data: &[i32]) -> bool {
    data.iter().any(|&x| x > 0)
}
```

## Lazy Iterator Methods

These methods return iterators (lazy):

| Method | Description |
|--------|-------------|
| `.filter()` | Keep matching elements |
| `.map()` | Transform elements |
| `.take(n)` | Limit to n elements |
| `.skip(n)` | Skip first n elements |
| `.zip()` | Pair with another iterator |
| `.chain()` | Concatenate iterators |
| `.flat_map()` | Map and flatten |
| `.enumerate()` | Add index |

## Consuming Methods

These methods consume the iterator (evaluate immediately):

| Method | Description |
|--------|-------------|
| `.collect()` | Gather into collection |
| `.for_each()` | Execute side effect |
| `.count()` | Count elements |
| `.sum()` | Sum elements |
| `.fold()` | Accumulate value |
| `.any()` | Check if any match |
| `.all()` | Check if all match |
| `.find()` | Find first match |

## Short-Circuit Benefits

```rust
// Without lazy: processes ALL items
let found: Vec<_> = items.iter()
    .filter(|x| expensive_check(x))
    .collect();
let result = found.first();

// With lazy: stops at first match
let result = items.iter()
    .find(|x| expensive_check(x));
```

## Pattern: Process Without Collecting

```rust
// Print all matches without allocating
data.iter()
    .filter(|x| x.is_valid())
    .for_each(|x| println!("{}", x));

// Count without collecting
let count = data.iter()
    .filter(|x| x.is_valid())
    .count();

// Sum without intermediate collection
let total: i64 = data.iter()
    .filter(|x| x.is_valid())
    .map(|x| x.value as i64)
    .sum();
```

## When Intermediate Collection Is Needed


```rust
// Need to iterate multiple times
let items: Vec<_> = data.iter()
    .filter(|x| x.is_valid())
    .collect();

let count = items.len();
let first = items.first();
for item in &items {
    process(item);
}

// Need to sort (requires concrete collection)
let mut sorted: Vec<_> = data.iter()
    .filter(|x| x.is_active)
    .collect();
sorted.sort_by_key(|x| x.priority);
```

## Pattern: Collect with Capacity


When you must collect, pre-allocate:

```rust
// With estimated capacity
let mut result = Vec::with_capacity(items.len());
result.extend(
    items.iter()
        .filter(|x| x.is_valid())
        .map(|x| x.clone())
);
```

## `take_while` Consumes The Item That Stopped It

`take_while` pulls the failing element from the underlying iterator to test it,
then discards it. When the stop condition is observed *on* an item that still
has to be kept — a terminator that belongs in the output, a record that must be
re-examined by the next stage — that element is silently gone, and it is gone
from the source iterator too, so a later `.next()` does not see it.

```rust
fn main() {
    let data = [1, 2, 3, 99, 4];

    // The sentinel 99 is tested, fails, and is dropped — it is not in the
    // output and it is not left in the iterator either.
    let mut it = data.iter().copied();
    let taken: Vec<_> = it.by_ref().take_while(|&n| n != 99).collect();
    assert_eq!(taken, vec![1, 2, 3]);
    assert_eq!(it.next(), Some(4), "99 was consumed, not left behind");

    // Keeping the boundary item: stop after including it.
    let mut kept = Vec::new();
    for n in data.iter().copied() {
        let last = n == 99;
        kept.push(n);
        if last {
            break;
        }
    }
    assert_eq!(kept, vec![1, 2, 3, 99]);
}
```

Reach for `by_ref()` plus an explicit loop, `peekable()` with `next_if`, or
`scan` when the boundary element matters. Assert on both the output's contents
and what remains in the source iterator — a test that checks only the taken
prefix passes either way.

## See Also

- [perf-iter-lazy](perf-iter-lazy.md) - keep iterators lazy and collect once
- [perf-iter-over-index](./perf-iter-over-index.md) - Prefer iterators
- [anti-collect-intermediate](./anti-collect-intermediate.md) - Anti-pattern
