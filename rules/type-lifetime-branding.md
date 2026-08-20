# type-lifetime-branding

> Mint a unique invariant lifetime with a higher-ranked closure so a handle from one instance cannot type-check against another

## Why It Matters

An index or key handed out by one collection is meaningless — or silently
wrong — when passed to a different collection of the same type. `arena_a` and
`arena_b` are both `Arena`, so `arena_b.get(handle_from_a)` type-checks and
reads whatever happens to live at that index. The usual defenses are runtime
ones — a generation counter, an owner id compared on every access — while
branding moves the check to compile time by giving each instance its own
*invariant* lifetime parameter, minted by a higher-ranked closure so no two
instantiations can ever unify. The cost is an unusual API shape — the collection is reachable
only inside a callback — and the benefit is that mixing handles stops being a
runtime error and becomes a type error.

## Minting And Marking The Brand

- Hand the branded value to the user through a
  `for<'brand> FnOnce(Arena<'brand>) -> R` callback. The higher-ranked bound
  is what mints a fresh lifetime per call; a plain generic parameter on the
  constructor lets the caller choose, and choosing means two instances can
  agree.
- Mark both the container and its handles with a marker that is **invariant**
  in the brand: `PhantomData<*mut &'brand ()>` (or `PhantomData<Cell<&'brand ()>>`).
  This is the load-bearing detail.
- Do **not** use `PhantomData<&'brand ()>`. That marker is covariant, so the
  compiler will happily shorten one brand to match another and the cross-instance
  mix compiles — the API looks branded and enforces nothing.
- Keep the brand parameter on every type that carries instance identity: the
  container, the handle, and any iterator or guard derived from them. A single
  un-branded intermediate launders the handle back into the unbranded world.
- Reach for this only when handle-mixing is a real hazard and the callback
  shape is acceptable. A generation counter costs a word and a branch and
  keeps an ordinary API; branding costs no runtime at all and costs API shape
  instead ([type-generational-handle](type-generational-handle.md)).

## Bad

```rust
pub struct Arena {
    items: Vec<String>,
}

/// A bare index carries no evidence of which arena produced it.
pub struct Handle(usize);

impl Arena {
    pub fn get(&self, handle: &Handle) -> &str {
        // `other_arena.get(&handle_from_this_one)` type-checks and reads
        // whatever occupies that index — or panics, on a good day.
        &self.items[handle.0]
    }
}
```

## Good

```rust
use std::marker::PhantomData;

/// The `*mut` marker makes `'brand` invariant. With a covariant
/// `PhantomData<&'brand ()>` here, the compiler shortens one brand to match
/// another and the cross-arena mix below compiles.
pub struct Arena<'brand> {
    items: Vec<String>,
    _brand: PhantomData<*mut &'brand ()>,
}

pub struct Handle<'brand> {
    index: usize,
    _brand: PhantomData<*mut &'brand ()>,
}

impl<'brand> Arena<'brand> {
    pub fn push(&mut self, value: String) -> Handle<'brand> {
        self.items.push(value);
        Handle { index: self.items.len() - 1, _brand: PhantomData }
    }

    pub fn get(&self, handle: Handle<'brand>) -> &str {
        // No bounds check needed for validity: a `Handle<'brand>` can only
        // have come from *this* arena.
        &self.items[handle.index]
    }
}

/// The higher-ranked bound mints a fresh `'brand` per call, so no two
/// arenas can ever share one.
pub fn with_arena<R>(f: impl for<'brand> FnOnce(Arena<'brand>) -> R) -> R {
    f(Arena { items: Vec::new(), _brand: PhantomData })
}

fn main() {
    let len = with_arena(|mut arena| {
        let handle = arena.push("hello".to_owned());
        arena.get(handle).len()
    });
    assert_eq!(len, 5);

    // Nesting two arenas and using the outer handle on the inner arena fails
    // to compile with E0521, citing invariance over `'brand`:
    //
    //   with_arena(|mut a| {
    //       let h = a.push("from A".to_owned());
    //       with_arena(|b: Arena| { let _ = b.get(h); });
    //   });
}
```

## Cases To Pin In Tests

- a handle used with the arena that produced it compiles and resolves;
- a handle from one arena used with a second, nested arena fails to compile —
  committed as a compile-fail case, since this is the entire guarantee;
- swapping the marker to the covariant `PhantomData<&'brand ()>` makes that
  compile-fail case start compiling, which is why the invariant marker is
  pinned by a test rather than a comment;
- a derived iterator or guard still carries the brand, so a handle cannot be
  laundered through it.

## See Also

- [type-generational-handle](type-generational-handle.md) - the runtime alternative, which keeps an ordinary API at the cost of a check
- [type-variance](type-variance.md) - why the `*mut` marker is invariant and `&'brand ()` is not
- [type-phantom-marker](type-phantom-marker.md) - the zero-sized marker mechanism this rule depends on
- [api-scoped-closure-access](api-scoped-closure-access.md) - the callback shape branding forces, and its other uses
- [test-compile-fail-guarantees](test-compile-fail-guarantees.md) - pinning a type-system-only guarantee with a committed compile-fail test
