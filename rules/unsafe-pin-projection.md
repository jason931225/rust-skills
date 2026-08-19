# unsafe-pin-projection

> Decide once whether each field of a `!Unpin` type is structurally pinned, and keep every accessor consistent with that choice

## Why It Matters

Pinning a struct says nothing about its fields. For each one the author must
decide whether the pin projects — whether `Pin<&mut Self>` yields
`Pin<&mut Field>` or plain `&mut Field` — and that decision carries
obligations: a structurally pinned field may never be moved out, never handed
out as `&mut`, and its `Drop` must not move it either. Mixing the two for one
field is unsound, and the mistake is invisible because both accessors compile.

## Bad

```rust
impl Machine {
    // Says the future is structurally pinned...
    fn future(self: Pin<&mut Self>) -> Pin<&mut Fut> {
        unsafe { self.map_unchecked_mut(|s| &mut s.future) }
    }

    // ...and then hands out a plain &mut to the same field, which lets a
    // caller `mem::replace` the pinned future out of the pinned struct
    fn future_mut(&mut self) -> &mut Fut {
        &mut self.future
    }
}
```

## Good

```rust
use std::marker::PhantomPinned;
use std::pin::Pin;

pub struct Machine {
    /// Structurally pinned: the address matters, so it is only ever reached
    /// through `Pin<&mut _>` and never moved out.
    buffer: [u8; 4],
    /// Not structurally pinned: an ordinary field, freely `&mut`-accessible.
    counter: u32,
    _pinned: PhantomPinned,
}

impl Machine {
    pub fn new() -> Self {
        Self { buffer: [0; 4], counter: 0, _pinned: PhantomPinned }
    }

    /// Projects the pin. The field is never exposed as a plain `&mut`.
    pub fn buffer(self: Pin<&mut Self>) -> Pin<&mut [u8; 4]> {
        // SAFETY: `buffer` is structurally pinned — no method moves out of it,
        // hands out `&mut` to it, or moves it in `Drop`.
        unsafe { self.map_unchecked_mut(|machine| &mut machine.buffer) }
    }

    /// Does not project: an unpinned field may be reached normally.
    pub fn counter(self: Pin<&mut Self>) -> &mut u32 {
        // SAFETY: `counter` is not structurally pinned, so handing out `&mut`
        // to it cannot invalidate any address-dependent invariant.
        unsafe { &mut self.get_unchecked_mut().counter }
    }
}

fn main() {
    let mut machine = Box::pin(Machine::new());

    *machine.as_mut().counter() += 1;
    assert_eq!(*machine.as_mut().counter(), 1);

    let buffer = machine.as_mut().buffer();
    // The pinned field is reachable only through the pin, so it cannot be
    // moved out — which is what keeps its address stable.
    assert_eq!(buffer.len(), 4);
}
```

## Key Points

- Write the classification down per field; a comment stating "structurally
  pinned" or "not pinned" is what a later reader and reviewer check against.
- A structurally pinned field forbids all four of: moving out, `&mut` access,
  `mem::replace`/`take`, and moving it during `Drop`.
- `Drop::drop` receives `&mut self` even for a `!Unpin` type, so a manual
  destructor must not move a pinned field out.
- Prefer a projection macro such as `pin-project` for anything non-trivial; it
  generates the accessors from the classification rather than trusting each one
  to be written correctly.
- A type with no address-dependent invariant needs none of this — it is `Unpin`
  and projection is irrelevant.

## See Also

- [unsafe-pin-address-stable](unsafe-pin-address-stable.md) - opting a type out of `Unpin` in the first place
- [unsafe-safety-comment](unsafe-safety-comment.md) - the per-block proof each projection needs
- [async-poll-contract](async-poll-contract.md) - where hand-written pinning usually appears
- [mem-take-replace](mem-take-replace.md) - the operation a structurally pinned field forbids
