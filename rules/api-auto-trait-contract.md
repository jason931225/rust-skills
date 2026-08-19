# api-auto-trait-contract

> Pin every auto trait a public type promises — `Send`, `Sync`, `Unpin`, `UnwindSafe` — with a compile-only assertion, so a private-field change that silently drops one is caught before release

## Why It Matters

`Send`, `Sync`, `Unpin`, and `UnwindSafe` are derived automatically from a
type's fields: the compiler grants them unless some field opts out. That
makes them part of a public type's contract without ever appearing in its
signature — callers write `spawn(work)` or store a value across an `.await`
because the type happens to be `Send`, not because anything documented it.
Adding a single non-`Send` field deep in a struct (a `Rc`, a raw pointer, a
`Cell`) silently removes that promise, and every downstream `spawn` call or
generic bound that depended on it fails to compile somewhere else in the
dependency graph — or, behind `impl Trait`, does not fail to compile at all
and instead loses a guarantee callers were relying on invisibly. Because the
compiler infers these traits rather than requiring an explicit `impl`,
nothing forces the author to notice the regression at the point where they
introduced it.

## Contract

- For every public type whose `Send`, `Sync`, or `Unpin` status is part of
  the contract (used across threads, held across an `.await`, or returned as
  `impl Trait` that leaks these through the existential), add a compile-only
  assertion function that names the bound explicitly.
- Write the assertion as a generic function with the required bounds, called
  once with the concrete type — `fn assert_bounds<T: Send + Sync>() {}` then
  `assert_bounds::<MyType>` — so a regression is a compile error at the
  assertion, not a mysterious failure at every call site downstream.
- Run the assertion in a test or a `const _: () = ...` context so it is part
  of the compiled crate, not documentation a reader has to trust.
- Treat a private-field change that removes an asserted auto trait as a
  breaking change for semver purposes, even though the field itself was never
  public.
- Do the same for a trait leaked through `-> impl Trait`: the concrete type
  behind the existential can gain or lose auto traits as its implementation
  changes, and the return position hides which ones it currently has.
- Do not assert traits the type does not need. An assertion is a promise to
  keep it true across every future change, not free documentation.

## Bad

```rust
// Depended on across threads via `spawn`, but nothing in this crate states
// or checks that `JobHandle` is `Send`. A later change — swapping the
// `Vec<u8>` buffer for an `Rc<[u8]>` to cut a clone — silently removes it,
// and the failure surfaces at every caller's `spawn` site instead of here.
pub struct JobHandle {
    id: u64,
    buffer: Vec<u8>,
}
```

## Good

```rust
pub struct JobHandle {
    id: u64,
    buffer: Vec<u8>,
}

/// Compiles only if every `T` here actually implements the listed bounds.
/// Never called at runtime — the call site below exists purely to force
/// monomorphization, which is where the compiler checks the bounds.
const fn assert_send_sync<T: Send + Sync>() {}

// A regression that makes `JobHandle` lose `Send` or `Sync` — for example,
// replacing `buffer: Vec<u8>` with `buffer: std::rc::Rc<[u8]>` — turns this
// line into a compile error naming exactly this assertion, instead of a
// failure at every unrelated `spawn` call across the dependency graph.
const _: () = assert_send_sync::<JobHandle>();

fn main() {
    assert_send_sync::<JobHandle>();
}
```

## Failure Tests

- the assertion compiles against the current definition of `JobHandle`;
- replacing `buffer`'s field type with a non-`Send` type (`Rc<[u8]>` in place
  of `Vec<u8>`) makes the assertion fail to compile, naming the assertion
  line rather than a distant caller;
- a type intentionally not `Send` (one wrapping a thread-affine handle) has
  no `Send` assertion, and a reviewer can see that absence is a decision, not
  an oversight, from the type's documentation;
- a function returning `-> impl Future<Output = T>` has its own assertion
  covering the returned type's `Send` status, separate from any assertion on
  `T` itself.

## See Also

- [err-send-sync-static](err-send-sync-static.md) - the same assertion pattern specialized for public error types
- [async-assert-send](async-assert-send.md) - the same pattern specialized for futures and task handles
- [proj-semver-contract](proj-semver-contract.md) - why a private-field regression here still counts as a breaking release
- [unsafe-send-sync-manual](unsafe-send-sync-manual.md) - the opposite direction: granting an auto trait by hand instead of losing one by accident
- [test-compile-fail-guarantees](test-compile-fail-guarantees.md) - pin the rejection case (a type that should not be `Send`) the same way
