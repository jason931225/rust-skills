# type-variance

> Keep generic types covariant where you can; reach for an extra lifetime parameter before accepting invariance

## Why It Matters

Variance decides which types may stand in for a type parameter. `&'a T` is
covariant in `'a` and in `T`, so a longer-lived or more-specific value is
accepted where a shorter-lived one is expected. `&mut T` is invariant in `T`,
and so is anything offering interior mutability — the compiler must refuse the
substitution, because a shorter-lived value written through the reference
would outlive its borrow. Invariance is not a bug, but it is contagious:
folding two independent lifetimes into one puts a lifetime behind a `&mut`,
and callers get borrow errors that look unrelated to the type they are using.

## Bad

```rust
struct MutStr<'a> {
    // One lifetime forces 'a to be both the outer borrow and the str's own
    // lifetime; behind &mut, that lifetime is invariant and cannot shorten.
    s: &'a mut &'a str,
}

fn main() {
    let mut s = "hello";
    *MutStr { s: &mut s }.s = "world";
    println!("{s}"); // error: s is still mutably borrowed
}
```

## Good

```rust
/// `'a` is the borrow of the slot; `'b` is the lifetime of the string it holds.
struct MutStr<'a, 'b> {
    s: &'a mut &'b str,
}

fn main() {
    let mut s = "hello";
    *MutStr { s: &mut s }.s = "world";
    // The outer borrow ends here, so the shared read below is fine.
    assert_eq!(s, "world");
}
```

With two parameters the compiler shortens only the outer borrow `'a` and
leaves `'b` alone. With one, it would have to shorten a lifetime sitting
behind `&mut`, which invariance forbids.

## Keeping Types Covariant

- Prefer shared references and owned values in public types; each `&mut T`
  field pins `T` invariantly.
- When a struct holds a reference to a borrowed value, give the outer borrow
  and the inner value separate lifetime parameters unless they are genuinely
  the same borrow.
- Weigh the two costs deliberately: one more lifetime parameter is cognitive
  overhead, invariance is an ergonomic cost paid by every caller.
- Function parameters are contravariant, so a callback that accepts a
  shorter-lived argument is more useful, not less; do not over-constrain
  callback signatures to `'static` out of habit.
- In types holding raw pointers, choose the `PhantomData` marker that states
  the intended variance and ownership rather than whichever one compiles.

## See Also

- [own-lifetime-elision](own-lifetime-elision.md) - add explicit lifetimes only where they buy something
- [type-phantom-marker](type-phantom-marker.md) - markers that declare variance and ownership
- [unsafe-send-sync-manual](unsafe-send-sync-manual.md) - variance is part of the soundness proof for pointer types
- [type-generic-bounds](type-generic-bounds.md) - do not constrain callers more than the code requires
