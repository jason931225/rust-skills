# perf-iter-over-index

> Traverse with iterators by default; keep indices when the index itself is part of the contract

## Why It Matters

An iterator states the traversal contract in the source: visit every element of
this collection, in order, exactly once. A manual `for i in 0..len` loop
restates that contract as arithmetic every later editor must re-verify — length
coupling between collections, off-by-one ranges, and repeated `data[i]` lookups
that must all agree. Performance is not the argument: LLVM can eliminate bounds
checks from an indexed loop and fail to vectorize an iterator chain, or the
reverse, so source syntax guarantees neither elision nor SIMD. Choose the form
that states the contract, and if a loop is hot, benchmark both and inspect the
optimized code before claiming either is faster.

## Bad

```rust
// Index exists only to reach the element, and each element is looked up twice.
fn sum_squares(data: &[i32]) -> i64 {
    let mut sum = 0i64;
    for i in 0..data.len() {
        sum += (data[i] as i64) * (data[i] as i64);
    }
    sum
}

// The loop bound silently truncates to the shorter input instead of stating
// whether unequal lengths are a caller error.
fn dot_product(a: &[f64], b: &[f64]) -> f64 {
    let mut sum = 0.0;
    for i in 0..a.len().min(b.len()) {
        sum += a[i] * b[i];
    }
    sum
}

// Hand-written range to visit every element in place.
fn double_values(data: &mut [i32]) {
    for i in 0..data.len() {
        data[i] *= 2;
    }
}
```

## Good

```rust
// Each element is named once; there is no range to get wrong.
fn sum_squares(data: &[i32]) -> i64 {
    data.iter()
        .map(|&x| (x as i64) * (x as i64))
        .sum()
}

// The length contract is explicit, and zip carries the pairing.
fn dot_product(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len(), "vectors must have equal length");
    a.iter()
        .zip(b.iter())
        .map(|(&x, &y)| x * y)
        .sum()
}

// In-place traversal with no index arithmetic.
fn double_values(data: &mut [i32]) {
    for x in data.iter_mut() {
        *x *= 2;
    }
}
```

## When Indexing Is Appropriate

Keep an index when the index is semantically required, when access is not a
single forward pass, or when a benchmark on the real workload shows the indexed
form is faster.

```rust
// The index is part of the output, so bind it explicitly.
for (i, value) in data.iter().enumerate() {
    println!("Index {}: {}", i, value);
}

// In-place interleaving: insertion positions are computed from two equal halves.
fn interleave_halves(data: &mut [i32]) {
    assert_eq!(data.len() % 2, 0, "interleaving requires two equal halves");
    let mid = data.len() / 2;
    for i in 0..mid {
        let target = i * 2 + 1;
        let source = mid + i;
        data[target..=source].rotate_right(1);
    }
}

// Several aligned buffers whose relationship is positional and asserted once.
fn blend(out: &mut [f32], a: &[f32], b: &[f32], weights: &[f32]) {
    assert!(out.len() == a.len() && a.len() == b.len() && b.len() == weights.len());
    for i in 0..out.len() {
        out[i] = a[i] * weights[i] + b[i] * (1.0 - weights[i]);
    }
}
```

## Contract Comparison

| Pattern | Source-level contract | Failure mode it removes |
|---------|-----------------------|-------------------------|
| `for i in 0..len` | Reader re-derives the valid range | — |
| `for &x in slice` | Visit every element once | Off-by-one, duplicated lookup |
| `.iter().enumerate()` | Value paired with its position | Index/value drift |
| `assert_eq!(a.len(), b.len()); a.iter().zip(b)` | Equal-length pairwise traversal | Silent length truncation |
| `data.swap(i, j)` | Deliberate positional access | Aliasing two `&mut` elements |

## Composition

```rust
// Adapters compose into one pass without an intermediate collection.
let result: Vec<_> = data.iter()
    .filter(|&&x| x > 0)
    .map(|x| x * 2)
    .collect();

// Short-circuits on the first match by definition, not by optimization.
let found = data.iter().any(|&x| x == target);

// Same traversal contract, parallel execution (with rayon).
use rayon::prelude::*;
let sum: i64 = data.par_iter().map(|&x| x as i64).sum();
```

## Verify Before Claiming a Win

```bash
cargo bench                                   # Measure the real workload
cargo asm --release my_crate::hot_function    # Read the whole loop, not one mnemonic
```

Rewriting an indexed loop as an iterator chain is a clarity change until a
benchmark says otherwise. If the indexed form measures faster on the target,
keep it and record the measurement next to the loop.

## See Also

- [perf-iter-lazy](./perf-iter-lazy.md) - Keep iterators lazy
- [opt-bounds-check](./opt-bounds-check.md) - Bounds check elimination
- [anti-index-over-iter](./anti-index-over-iter.md) - Anti-pattern
- [conc-rayon-par-iter](./conc-rayon-par-iter.md) - Parallelize data-parallel loops
