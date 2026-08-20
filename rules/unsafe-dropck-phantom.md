# unsafe-dropck-phantom

> Add `PhantomData<T>` to any type that drops or accesses a `T` only through a raw pointer, so the borrow checker knows it and rejects programs that let `T` expire too early

## Why It Matters

The borrow checker's drop-check analysis decides how long a generic
parameter's borrows must remain valid by looking at what the type's fields
declare, not at what its `Drop` implementation actually does: a field typed
`T` or `&T` tells it "this value is used when the struct drops" and requires
any borrowed data in `T` to still be valid then, while a field typed `*mut T`
or `*const T` tells it nothing, since raw pointers carry no ownership or
borrow information. A struct that stores a `*mut T`, leaks a `Box<T>` to get
it, and reclaims and drops that `T` in its own `Drop` impl is therefore
invisible to the analysis — the borrow checker will happily accept a caller
that lets `T`'s borrowed data end before the struct drops, because nothing in
the struct's declared fields says otherwise. The fix is not the `unsafe`
block that reclaims the pointer — it is a `PhantomData<T>` field that restores
the honest declaration drop-check needs.

## PhantomData Marker Requirements

- If a type's `Drop` implementation dereferences, drops, or otherwise touches
  a `T` reached only through a raw pointer, add a `PhantomData<T>` field (or
  `PhantomData<*const T>`/`PhantomData<*mut T>` when only the pointee's
  existence matters, not ownership) so drop-check treats the type as if it
  held a `T` directly.
- Choose the `PhantomData` variant for the ownership and variance the type
  actually has: `PhantomData<T>` claims ownership and drop-check pessimism;
  `PhantomData<*const T>` claims neither ownership nor a lifetime obligation,
  only that the pointee type parameter is used.
- Do not add the marker and stop there — the field only fixes what drop-check
  assumes; the raw-pointer reclaim in `Drop` still needs its own `// SAFETY:`
  proof that the pointer is valid and reclaimed exactly once.
- Prefer storing `T` or `Box<T>` directly when nothing forces a raw pointer.
  The marker is a repair for cases — custom allocators, self-referential
  layouts, FFI handles — where a raw pointer is the field type for other
  reasons, not a default habit.
- When a type intentionally wants to be usable with expired borrows in `T`
  (the type never actually touches `T` on drop), the missing `PhantomData` is
  correct; document why the type is exempt so a later change that starts
  touching `T` in `Drop` does not silently reintroduce the gap.

## Bad

```rust
/// Leaks a `Box<T>` to hold it behind a raw pointer, and reclaims it in
/// `Drop` — but declares no field that mentions `T`, so drop-check has no
/// reason to require `T`'s borrows to outlive this type. A caller can build
/// one with a `T` that borrows local data and let that data end before this
/// value drops; the borrow checker accepts it, and `Drop` then touches a
/// dangling reference through the reclaimed `T`.
pub struct Storage<T> {
    ptr: *mut T,
}
```

## Good

```rust
use std::marker::PhantomData;

/// Same raw-pointer storage, plus a `PhantomData<T>` field. Drop-check now
/// sees a type that owns and drops a `T`, and requires `T`'s borrows to
/// remain valid for as long as a `Storage<T>` carrying them might exist —
/// exactly the obligation the raw pointer alone could not express.
pub struct Storage<T> {
    ptr: *mut T,
    _owns_t: PhantomData<T>,
}

impl<T> Storage<T> {
    pub fn new(value: T) -> Self {
        Storage { ptr: Box::into_raw(Box::new(value)), _owns_t: PhantomData }
    }

    pub fn get(&self) -> &T {
        // SAFETY: `ptr` was produced by `Box::into_raw` in `new` and is
        // reclaimed exactly once, in `Drop::drop`, so it stays valid for
        // every call to `get` in between.
        unsafe { &*self.ptr }
    }
}

impl<T> Drop for Storage<T> {
    fn drop(&mut self) {
        // SAFETY: `ptr` was produced by `Box::into_raw` in `new`, is unique
        // to this `Storage`, and this is the only place it is reclaimed.
        drop(unsafe { Box::from_raw(self.ptr) });
    }
}

fn main() {
    use std::cell::Cell;

    // A drop marker that records into a `Cell` reachable after `Storage`
    // has been dropped, so the assertion below can observe that the T's
    // destructor really ran, and ran exactly once.
    struct Recorder<'a>(&'a Cell<u32>);
    impl Drop for Recorder<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    let drops = Cell::new(0);
    {
        let storage = Storage::new(Recorder(&drops));
        assert_eq!(storage.get().0.get(), 0);
    } // `Storage::drop` runs here, reclaiming and dropping the `Recorder`.
    assert_eq!(drops.get(), 1, "the raw-pointer-held value was dropped exactly once");
}
```

## Drop Behavior To Verify

- the recorder's destructor runs exactly once when `Storage` goes out of
  scope, proving the raw pointer is reclaimed rather than leaked or double-freed;
- `Storage::get` reads through the pointer successfully before drop, proving
  the value stored via `Box::into_raw` round-trips correctly;
- a type audit confirms every field that is only a raw pointer to `T` has a
  matching `PhantomData<T>` (or the narrower pointer variant) documenting the
  ownership drop-check should assume;
- a type intentionally exempt from this — one whose `Drop` never touches
  `T` — carries a comment saying so, not a silently absent marker.

## See Also

- [type-phantom-marker](type-phantom-marker.md) - the general `PhantomData` mechanism this rule specializes for drop-check
- [unsafe-safety-comment](unsafe-safety-comment.md) - the proof the raw-pointer reclaim in `Drop` still needs
- [mem-drop-order](mem-drop-order.md) - the ordering guarantee drop-check is protecting once the marker states the true ownership
- [unsafe-pointer-provenance](unsafe-pointer-provenance.md) - the pointer arithmetic obligations for any raw pointer this type carries
- [test-compile-fail-guarantees](test-compile-fail-guarantees.md) - pin the rejection (a `T` that cannot outlive this type) with a committed compile-fail case
