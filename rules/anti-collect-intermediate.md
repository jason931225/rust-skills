# anti-collect-intermediate

> Don't collect intermediate iterators

## Why It Matters

Collecting into an owned container such as `Vec`, `String`, or `HashMap`
materializes intermediate state and commonly allocates. It also ends iterator
fusion before the next pass. Keep a transform lazy while the next consumer only
needs iteration, but collect deliberately for sorting, random access, repeated
passes, ownership, or an API boundary. `collect()` itself is generic and does
not universally allocate—for example, collecting into `Result` short-circuits.

## Bad

```rust
// Three allocations, three passes
fn process(data: Vec<i32>) -> Vec<i32> {
    let step1: Vec<_> = data.into_iter()
        .filter(|x| *x > 0)
        .collect();
    
    let step2: Vec<_> = step1.into_iter()
        .map(|x| x * 2)
        .collect();
    
    step2.into_iter()
        .filter(|x| *x < 100)
        .collect()
}

// Collecting just to check length
fn has_valid_items(items: &[Item]) -> bool {
    let valid: Vec<_> = items.iter()
        .filter(|i| i.is_valid())
        .collect();
    !valid.is_empty()
}

// Collecting to iterate again
fn sum_valid(items: &[Item]) -> i64 {
    let valid: Vec<_> = items.iter()
        .filter(|i| i.is_valid())
        .collect();
    valid.iter().map(|i| i.value).sum()
}
```

## Good

```rust
// Single allocation, single pass
fn process(data: Vec<i32>) -> Vec<i32> {
    data.into_iter()
        .filter(|x| *x > 0)
        .map(|x| x * 2)
        .filter(|x| *x < 100)
        .collect()
}

// No allocation - iterator short-circuits
fn has_valid_items(items: &[Item]) -> bool {
    items.iter().any(|i| i.is_valid())
}

// No intermediate allocation
fn sum_valid(items: &[Item]) -> i64 {
    items.iter()
        .filter(|i| i.is_valid())
        .map(|i| i.value)
        .sum()
}
```

## When Collection Is Needed

```rust
// Need to iterate twice
let valid: Vec<_> = items.iter()
    .filter(|i| i.is_valid())
    .collect();
let count = valid.len();
for item in &valid {
    process(item);
}

// Need to sort (requires concrete collection)
let mut sorted: Vec<_> = items.iter()
    .filter(|i| i.is_active())
    .collect();
sorted.sort_by_key(|i| i.priority);

// Need random access
let indexed: Vec<_> = items.iter().collect();
let middle = indexed.get(indexed.len() / 2);
```

## Iterator Methods That Avoid Collection

| Instead of Collecting to... | Use |
|-----------------------------|-----|
| Check if non-empty | `.any(|_| true)` or `.next().is_some()` |
| Check if any match | `.any(predicate)` |
| Check if all match | `.all(predicate)` |
| Count elements | `.count()` |
| Sum elements | `.sum()` |
| Find first | `.find(predicate)` |
| Get first | `.next()` |
| Get last | `.last()` |

## Pattern: Deferred Collection

```rust
// Return iterator, let caller collect if needed
fn valid_items(items: &[Item]) -> impl Iterator<Item = &Item> {
    items.iter().filter(|i| i.is_valid())
}

// Caller decides
let count = valid_items(&items).count();  // No collection
let vec: Vec<_> = valid_items(&items).collect();  // Collection when needed
```

## Comparison

| Pattern | Allocations | Passes |
|---------|-------------|--------|
| `.collect()` each step | N | N |
| Single chain, one `.collect()` | 1 | 1 |
| No collection (streaming) | 0 | 1 |

## Editing A Collection Is Not Rebuilding It

When the result should be the collection you already have, `iter_mut`,
`retain`, and the in-place sorts say so directly. `collect` builds a second
collection, which is a different statement about what the code is doing:

```rust
fn main() {
    let mut load = vec![10u64, 20, 30, 40];

    // Editing: the same Vec, still the same buffer.
    for value in &mut load {
        *value *= 2;
    }
    assert_eq!(load, vec![20, 40, 60, 80]);

    // Editing: `retain` removes in place rather than filtering into a new Vec.
    load.retain(|value| *value >= 40);
    assert_eq!(load, vec![40, 60, 80]);
}
```

The allocation argument is real but narrower than it is usually stated, so it
is worth stating correctly. A **borrowing** collect allocates a second buffer;
a **consuming** collect whose element type keeps the same layout reuses the
original through `Vec`'s in-place specialization; changing the element width
defeats that reuse:

```text
consuming same-width reused buffer: true
borrowing reused buffer:            false
width-change reused buffer:         false
```

So `v = v.into_iter().map(..).collect()` is not the allocation people warn
about, and `iter_mut().for_each(..)` is not faster than a `for` loop — neither
allocates. What actually distinguishes the forms is what they claim: editing
says the collection persists and its identity matters, rebuilding says a new
value is being produced. Reach for `collect` when you want a different
collection, a different type, or a different length that in-place editing
cannot express.

## See Also

- [perf-iter-lazy](perf-iter-lazy.md) - keep iterators lazy and collect once
- [perf-iter-lazy](./perf-iter-lazy.md) - Lazy evaluation
- [perf-iter-over-index](./perf-iter-over-index.md) - Iterator patterns
