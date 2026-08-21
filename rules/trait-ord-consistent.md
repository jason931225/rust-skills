# trait-ord-consistent

> Keep `Ord`, `PartialOrd`, `Eq`, and `PartialEq` consistent

## Why It Matters

Ordered collections assume that comparison defines one total order. If `a.cmp(&b) == Ordering::Equal` disagrees with `a == b`, a `BTreeMap` can lose entries, return the wrong value, or panic after an internal optimization changes. Rust 1.96 optimized `BTreeMap::append`, exposing more incorrect `Ord` implementations; the implementation was already a logic error. Derive the comparison traits together whenever field order expresses the intended key.

## Bad

```rust
use std::cmp::Ordering;

#[derive(Debug)]
struct Job {
    id: u64,
    priority: u8,
}

impl PartialEq for Job {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Job {}

impl Ord for Job {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority.cmp(&other.priority)
    }
}
impl PartialOrd for Job {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// Same priority compares Equal even when the IDs — and Eq — differ.
```

## Good

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct JobKey {
    priority: u8,
    id: u64,
}

#[derive(Debug)]
struct Job {
    key: JobKey,
    payload: String,
}
```

If comparison must be manual, implement every relation from the same key:

```rust
use std::cmp::Ordering;

#[derive(Debug)]
struct JobKey {
    id: u64,
    priority: u8,
}

impl JobKey {
    fn order_key(&self) -> (u8, u64) {
        (self.priority, self.id)
    }
}

impl PartialEq for JobKey {
    fn eq(&self, other: &Self) -> bool {
        self.order_key() == other.order_key()
    }
}
impl Eq for JobKey {}

impl Ord for JobKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.order_key().cmp(&other.order_key())
    }
}
impl PartialOrd for JobKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
```

## Ordering Consistency Requirements

- For every `a` and `b`, `a.cmp(&b) == Ordering::Equal` must be equivalent to `a == b`.
- The order must be reflexive, antisymmetric, and transitive. Test these properties for manual implementations.
- Do not implement `Ord` for domains with a genuine partial order. Floating-point values require an explicit policy such as `total_cmp` or a validated non-NaN wrapper.
- Keep mutable payload outside the ordered key. Mutating a key while it is inside an ordered collection violates the collection's invariants.

## What Derived Ordering Actually Compares

For a struct, `#[derive(PartialOrd, Ord)]` compares fields top to bottom. For a
fieldless enum it compares **discriminant values** — which is not the same as
declaration order, though the two coincide in the common case where no
discriminants are written and they ascend from zero.

Assigning explicit discriminants breaks the coincidence, silently:

```rust
#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum Plain {
    A,
    B,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum Explicit {
    A = 5,
    B = 1,
}

fn main() {
    assert!(Plain::A < Plain::B, "no discriminants: source order decides");
    assert!(Explicit::B < Explicit::A, "explicit values decide instead");
}
```

That interacts badly with pinning discriminants for a wire or hardware
contract, because the two reasons to write a number are unrelated: one is what
an external system requires, the other is what "greater" should mean. A type
that needs both owes an explicit check that its declared values ascend in the
same direction as its meaning — or a hand-written `Ord` that says so directly
rather than a derive that happens to agree.

Either way the ordering is a property of the source, so reordering variants or
renumbering them changes every comparison, every `max`, and every `sort`.

Where the order is meaningful — severity levels, log levels, protocol
versions — say so at the type, so the next person to alphabetise the variants
has been warned. Where it is not meaningful, derive `PartialEq`/`Eq` alone and
leave the type unordered rather than shipping an order nobody chose.

## See Also

- [type-newtype-ids](type-newtype-ids.md) - give identifiers a dedicated key type
- [num-float-compare](num-float-compare.md) - use `total_cmp` when floats need a total order
- [trait-default-methods](trait-default-methods.md) - preserve trait contracts when overriding behavior
