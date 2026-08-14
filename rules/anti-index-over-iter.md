# anti-index-over-iter

> Don't use indexing when iterators work

## Why It Matters

Manual indexing (`for i in 0..len`) exposes bounds and length coordination at
every access, which increases off-by-one and mismatched-slice risk. The
optimizer can eliminate bounds checks and vectorize either indexed or iterator
code; source syntax does not guarantee either result. Prefer iterators because
they express traversal and pairing directly, then inspect generated code or
benchmark when the loop is actually hot.

## Bad

```rust
// Manual indexing - index and length coordination is hand-written
fn sum_squares(data: &[i32]) -> i64 {
    let mut result = 0i64;
    for i in 0..data.len() {
        result += (data[i] as i64) * (data[i] as i64);
    }
    result
}

// Index-based with multiple arrays
fn dot_product(a: &[f64], b: &[f64]) -> f64 {
    let mut sum = 0.0;
    for i in 0..a.len().min(b.len()) {
        sum += a[i] * b[i];
    }
    sum
}

// Mutation with indices
fn normalize(data: &mut [f64]) {
    let max = data.iter().cloned().fold(0.0, f64::max);
    for i in 0..data.len() {
        data[i] /= max;
    }
}
```

## Good

```rust
// Iterator expresses traversal without manual bounds coordination
fn sum_squares(data: &[i32]) -> i64 {
    data.iter()
        .map(|&x| (x as i64) * (x as i64))
        .sum()
}

// Zip stops at the shorter input, so state the length contract explicitly
fn dot_product(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len(), "dot product operands must have equal length");
    a.iter()
        .zip(b.iter())
        .map(|(&x, &y)| x * y)
        .sum()
}

// Mutable iteration
fn normalize(data: &mut [f64]) {
    let max = data.iter().cloned().fold(0.0, f64::max);
    for x in data.iter_mut() {
        *x /= max;
    }
}
```

## When Indices Are Needed

Sometimes you genuinely need indices:

```rust
// Need index in output
for (i, item) in items.iter().enumerate() {
    println!("{}: {}", i, item);
}

// Non-sequential access: `swap` is the positional API, because two indexed
// mutable borrows of the same slice in one call do not compile
for i in (0..data.len().saturating_sub(1)).step_by(2) {
    data.swap(i, i + 1);
}

// Multi-dimensional iteration
for i in 0..rows {
    for j in 0..cols {
        matrix[i][j] = i * cols + j;
    }
}
```

## Comparison

| Pattern | Source-level contract | Optimization |
|---------|-----------------------|--------------|
| `for i in 0..len { data[i] }` | Coordinates index and bounds manually | Bounds checks may be eliminated |
| `for x in &data` | Traverses existing elements | Often optimized to the same loop |
| `for x in data.iter()` | Traverses existing elements | Often optimized to the same loop |
| `data.iter().enumerate()` | Couples each value to its index | Often optimized to the same loop |

## Common Conversions

| Index Pattern | Iterator Pattern |
|---------------|------------------|
| `for i in 0..v.len()` | `for x in &v` |
| `v[0]` | `v.first()` |
| `v[v.len()-1]` | `v.last()` |
| `for i in 0..a.len() { a[i] + b[i] }` | `a.iter().zip(&b)` (assert equal lengths first) |
| `for i in 0..v.len() { v[i] *= 2 }` | `for x in &mut v { *x *= 2 }` |

## Performance Note

Both forms below usually compile to the same loop; neither syntax guarantees
bounds check elimination or vectorization. Decide with a benchmark and by
inspecting the generated code, not by the shape of the source.

```rust
let sum: i32 = data.iter().sum();

let mut sum = 0;
for i in 0..data.len() {
    sum += data[i];
}
```

## See Also

- [perf-iter-over-index](./perf-iter-over-index.md) - Traversal contract and when indices are genuinely needed
- [opt-bounds-check](./opt-bounds-check.md) - Bounds check elimination
- [perf-iter-lazy](./perf-iter-lazy.md) - Lazy iterators
