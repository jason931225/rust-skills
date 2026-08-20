# const-fn

> Make functions `const fn` when they can run at compile time

## Why It Matters

A `const fn` can be called in const contexts—array lengths, `const`/`static`
initializers, and const-generic arguments—as well as at runtime. Marking a
function `const` expands its public contract: calls required by a const context
are evaluated during compilation, while ordinary calls retain normal runtime
semantics and optimization. Const-evaluable operations evolve by Rust release,
so use only capabilities supported by the crate's MSRV.

## Bad

```rust
// not const — cannot use result as an array length or const initializer
fn header_len() -> usize {
    4
}

fn magic_mask() -> u32 {
    0xFF00_FF00
}

fn make_buf() -> [u8; 8] {
    // A non-const call is not allowed in an array length.
    [0u8; header_len()]  // error: `header_len` is not a `const fn`
}
```

## Good

```rust
const fn header_len() -> usize {
    4
}

const fn magic_mask() -> u32 {
    0xFF00_FF00
}

// usable as an array length — evaluated at compile time
let buf = [0u8; header_len()];

// usable in a const initializer
const MASK: u32 = magic_mask();

// usable in a static
static HEADER: [u8; header_len()] = [0u8; header_len()];

// const fn with logic — still fine on stable
const fn align_up(n: usize, align: usize) -> Option<usize> {
    if !align.is_power_of_two() {
        return None;
    }
    let mask = align - 1;
    match n.checked_add(mask) {
        Some(sum) => Some(sum & !mask),
        None => None,
    }
}

const ALIGNED: Option<usize> = align_up(13, 8); // Some(16)
```

## Constructor-Enforced Relational Invariants

A `const fn` constructor is not limited to validating one value in
isolation — it can assert relationships *among* several constants, and a
failed `assert!` there is a compile error, not a runtime check that a
missed test path never exercises. This matters most for layouts with
cross-value invariants: memory regions that must not overlap and must stay
within a bound, protocol fields that must not leave gaps, a multi-stage
derivation (a PLL configuration, a baud-rate divisor chain) where an
intermediate value has its own valid range independent of the final result.

```rust
struct MemoryRegion {
    start: usize,
    size: usize,
}

const fn checked_layout(regions: &[MemoryRegion], total: usize) -> bool {
    let mut i = 0;
    while i < regions.len() {
        let end = regions[i].start + regions[i].size;
        if end > total {
            return false; // a region overruns the physical bound
        }
        let mut j = i + 1;
        while j < regions.len() {
            let other_end = regions[j].start + regions[j].size;
            let overlaps = regions[i].start < other_end && regions[j].start < end;
            if overlaps {
                return false; // two regions claim the same bytes
            }
            j += 1;
        }
        i += 1;
    }
    true
}

const REGIONS: [MemoryRegion; 2] =
    [MemoryRegion { start: 0, size: 64 }, MemoryRegion { start: 64, size: 64 }];
const _: () = assert!(checked_layout(&REGIONS, 128), "region layout is invalid");
```

For a sequential layout (a wire frame, a packed struct's fields), derive
each field's position from the *previous* field's end instead of writing
independent offsets — a fifth field with an independent offset needs four
new pairwise-overlap assertions to stay honest, while a derived offset makes
a gap or overlap structurally unwritable:

```rust
struct Field {
    offset: usize,
    size: usize,
}

impl Field {
    const fn first(size: usize) -> Self {
        Field { offset: 0, size }
    }

    const fn then(&self, size: usize) -> Self {
        Field { offset: self.offset + self.size, size }
    }

    const fn end(&self) -> usize {
        self.offset + self.size
    }
}

const HEADER: Field = Field::first(4);
const PAYLOAD: Field = HEADER.then(64);
const CRC: Field = PAYLOAD.then(2);
const MAX_FRAME: usize = 128;
const _: () = assert!(CRC.end() <= MAX_FRAME, "frame layout exceeds the maximum size");
```

When a `const fn` derives a value through several stages (each depending on
the last), assert every intermediate against its own valid range inside the
same function, not only the final result — the compiler error then names
the broken stage, and a later change to one input cannot silently violate a
mid-chain limit that a single end-to-end check would miss.

## Stability And MSRV

Adding `const` is generally backwards-compatible. Removing it later can break
callers that use the function in const contexts, so treat const-evaluability as
part of a public API's compatibility surface. Add it when the supported MSRV
can express the durable implementation; do not promise it speculatively and
then remove it when a non-const operation becomes convenient.

## See Also

- [const-block](const-block.md) - force compile-time evaluation and assertions inline
- [const-generics](const-generics.md) - parameterize types and functions over const values
- [opt-inline-small](opt-inline-small.md) - inline small hot functions
