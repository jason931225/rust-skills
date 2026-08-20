# own-mutation-scope

> Confine mutation to the block that builds the value, and bind the result immutably

## Why It Matters

`let mut` is usually written once and then applies for the rest of the scope,
long after the value has stopped changing. Every later reader has to scan
forward to find out whether it still changes, and every later editor is free to
make it change again. A block expression ends the mutability where the
construction ends: the `mut` binding lives inside, the block yields the
finished value, and what escapes is immutable by type rather than by
convention.

## Bad

```rust
pub fn ranked(scores: &[u32]) -> Vec<u32> {
    // `mut` from here to the end of the function, though the sorting is over
    // three lines in. Nothing stops a later edit from pushing to it.
    let mut ranked = scores.to_vec();
    ranked.sort_unstable();
    ranked.dedup();

    // ... fifty lines of unrelated work, all of it able to mutate `ranked` ...

    ranked
}
```

## Good

```rust
pub fn ranked(scores: &[u32]) -> Vec<u32> {
    // Mutation is scoped to the construction. `ranked` is immutable after it.
    let ranked = {
        let mut working = scores.to_vec();
        working.sort_unstable();
        working.dedup();
        working
    };

    // Any attempt to mutate `ranked` from here on is a compile error.
    ranked
}

fn main() {
    assert_eq!(ranked(&[3, 1, 3, 2]), vec![1, 2, 3]);
}
```

## Constructions With No Single Pipeline

The obvious objection is that iterators already do this — and where a single
chain expresses the construction, it should. This form is for the cases where
one does not:

- **Build then finalise.** Sorting, deduplicating, truncating, or reserving
  operate on the collection rather than on its elements, so they follow the
  collection into existence rather than composing into its construction.
- **Termination depends on accumulated state.** A loop that stops once a total
  is reached is not `take_while`, which decides from the element alone and
  drops the element that ended it.
- **Several fields assembled with interdependencies.** A struct whose later
  fields are computed from earlier ones is a sequence of statements; a builder
  is the answer when that sequence is public API, and a block is the answer
  when it is local.

Two limits worth keeping. If the block grows past a screen it wants to be a
function, where the same guarantee comes from the signature and the name says
what was built. And this is about locals, not fields: a struct field that
changes only during construction is a job for a constructor that takes the
finished value, not for a block.

## See Also

- [mem-shrink-to-fit](mem-shrink-to-fit.md) - the type-level version, freezing a grown collection into a boxed slice
- [perf-iter-lazy](perf-iter-lazy.md) - the chain to prefer when the construction is a single pipeline
- [api-builder-pattern](api-builder-pattern.md) - the same staging when it is public API rather than a local
- [own-borrow-over-clone](own-borrow-over-clone.md) - what the immutable binding then allows callers to do
