# proj-avoid-statics

> Do not store mutable or process-identity state in `static`; pass it in. Reserve `static` for immutable tables

## Why It Matters

A `static` looks unique, but Cargo can link multiple versions of one crate and
give each copy its own counters, registries, or logger. Mutable statics also
couple tests and can create lock or cache-line contention, so pass cell-local
state through a service handle. Use a `static` only when a second copy cannot
change the answer, such as a lookup table, interned string, or atomic fast
path.

The same identity warning applies to thread-local state: every linked crate
version and thread gets another copy, especially across incompatible `0.x`
minor lines; Edition 2024 denies *references* to `static mut` (`static_mut_refs`); direct access still compiles but every use needs `unsafe`, and workspace lint policy can
keep ad-hoc globals out of libraries.

## Bad

```rust
use std::sync::atomic::{AtomicUsize, Ordering};

static HITS: AtomicUsize = AtomicUsize::new(0);

pub fn record_hit() -> usize {
    HITS.fetch_add(1, Ordering::Relaxed)
}
```

## Good

```rust
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct HitCounter {
    hits: AtomicUsize,
}

impl HitCounter {
    pub const fn new() -> Self {
        Self {
            hits: AtomicUsize::new(0),
        }
    }

    pub fn record(&self) -> usize {
        self.hits.fetch_add(1, Ordering::Relaxed)
    }
}

// Immutable table: a second copy would still return the same bytes.
static CRC_NIBBLE: [u8; 16] = [0, 1, 1, 0, 1, 0, 0, 1, 1, 0, 0, 1, 0, 1, 1, 0];

pub fn parity_nibble(n: u8) -> u8 {
    CRC_NIBBLE[(n & 0x0f) as usize]
}

fn main() {
    let counter = HitCounter::new();
    assert_eq!(counter.record(), 0);
    assert_eq!(parity_nibble(3), 0);
}
```

## See Also

- [const-vs-static](const-vs-static.md) - `const` for inlined values; `static` only when you need an address
- [conc-thread-local](conc-thread-local.md) - thread-locals are still process-global per thread
- [test-mock-traits](test-mock-traits.md) - inject clocks, entropy, and I/O instead of reading them from a static
