# perf-profile-first

> Profile before optimizing

## Why It Matters

Intuition about performance is often wrong, and actual bottlenecks may hide
outside the code that looks slow. Profiling identifies where time and
allocations are spent before optimization begins.

Decide early whether the crate affects fleet cost, throughput, or latency. If it
does, identify hot paths and benchmark them under representative load, then
record their budgets near the benchmark or in contributor guidance so later
changes do not silently move work back into them.

## Bad

```rust
// Optimizing without measuring
fn process(data: &[Item]) -> Vec<Output> {
    // "I bet this clone is slow..."
    let cloned: Vec<_> = data.iter().cloned().collect();

    // Actually, 99% of time is spent here:
    cloned.iter().map(|x| expensive_computation(x)).collect()
}

// Over-engineering rarely-called code
#[inline(always)]
fn rarely_called() {
    // This runs once at startup...
}
```

## Good

```rust
use rayon::prelude::*;

// 1. Profile first
// cargo flamegraph --bin myapp
// cargo instruments -t time --bin myapp (macOS)

// 2. Find the actual bottleneck
// Flamegraph shows expensive_computation takes 95% of time

// 3. Optimize the hot spot
fn process(data: &[u64]) -> Vec<u64> {
    // Clone is fine - only 1% of time
    let cloned: Vec<_> = data.iter().cloned().collect();

    // Focus optimization HERE
    cloned.par_iter()
        .map(|x| expensive_computation(x))
        .collect()
}

fn expensive_computation(value: &u64) -> u64 {
    value.wrapping_mul(*value)
}
```

## Profiling Tools

### Flamegraphs (Recommended Start)

```bash
# Install
cargo install flamegraph

# Profile
cargo flamegraph --bin myapp -- <args>

# Opens flamegraph.svg showing call stacks by time
```

### perf (Linux)

```bash
# Build first so perf records the product binary rather than Cargo and rustc.
cargo build --release
perf record --call-graph dwarf ./target/release/myapp <args>

# Report
perf report

# Or generate flamegraph
perf script | inferno-collapse-perf | inferno-flamegraph > flamegraph.svg
```

### Instruments (macOS)

```bash
# Install cargo-instruments
cargo install cargo-instruments

# Time profiler
cargo instruments -t time --release

# Allocations profiler
cargo instruments -t alloc --release
```

### DHAT (Heap Profiling)

```rust
#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn main() {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();
    // ... your code
}
```

```bash
# The feature keeps profiling allocation and output out of ordinary builds.
cargo run --release --features dhat-heap
# Review dhat-heap.json.
```

### criterion (Micro-benchmarks)

```rust
use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;

fn bench_my_function(c: &mut Criterion) {
    let input = 42_u64;
    c.bench_function("my_function", |b| {
        b.iter(|| my_function(black_box(input)))
    });
}

fn my_function(value: u64) -> u64 {
    value.wrapping_mul(value)
}

criterion_group!(benches, bench_my_function);
criterion_main!(benches);
```

Register the benchmark harness and keep symbols in optimized benchmark and
release builds so profilers can attribute the deployed work:

```toml
[[bench]]
name = "my_function"
harness = false

[profile.bench]
debug = 1

[profile.release]
debug = 1
```

Record and compare a reviewed benchmark baseline in CI for budgeted paths:

```bash
cargo bench --bench my_function -- --save-baseline admitted
cargo bench --bench my_function -- --baseline admitted
```

For fleet-cost or tail-latency services, supplement laboratory profiles with
bounded continuous profiling of the deployed artifact.

## What to Look For

```
Flamegraph Reading:
├── Width = time spent
├── Height = call stack depth
└── Look for:
    ├── Wide bars (time hogs)
    ├── malloc/free (allocation heavy)
    ├── memcpy (copying data)
    └── Unexpected functions taking time
```

## Common Findings

Treat each common finding as a hypothesis and re-profile after changing it:

- For hash-heavy trusted keys, benchmark a faster hasher. Keep a HashDoS-
  resistant hasher for attacker-controlled keys.
- For allocator-heavy string paths, test capacity planning, borrowing, or
  representation changes against the measured lifetime.
- For clone-heavy paths, confirm ownership and lifetime changes reduce total
  work rather than merely moving it.
- For bounds checks visible in generated code, try safe iterator or chunk
  structure before considering unchecked access.
- For lock contention, shorten or shard the critical section. Try `RwLock`
  only when measured read dominance and critical-section duration justify its
  writer-fairness trade-off.

## Optimization Workflow

```
1. Decide whether performance or unit cost is a product constraint
2. Write correct code first
3. Identify and document the sensitive paths and budgets
4. Write benchmarks for those paths
5. Profile CPU and allocations under realistic load
6. Optimize ONE measured bottleneck
7. Measure improvement
8. Repeat if needed
```

## Evidence: Rust Performance Book

> "The biggest performance improvements often come from changes to algorithms or data structures, rather than low-level optimizations."

> "It is worth understanding which Rust data structures and operations cause allocations, because avoiding them can greatly improve performance."

Sources: [General Tips](https://nnethercote.github.io/perf-book/general-tips.html)
and [Heap Allocations](https://nnethercote.github.io/perf-book/heap-allocations.html).

## See Also

- [opt-lto-release](opt-lto-release.md) - evaluate LTO for release builds
- [test-criterion-bench](test-criterion-bench.md) - use criterion for benchmarking
- [anti-premature-optimize](anti-premature-optimize.md) - do not optimize without data
- [perf-global-allocator](perf-global-allocator.md) - pick the process allocator after a profile, in main.rs
