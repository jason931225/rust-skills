# unsafe-pin-address-stable

> Opt address-dependent types out of `Unpin` with `PhantomPinned` and expose their mutation only through `Pin<&mut Self>`

## Why It Matters

`Pin` constrains nothing by itself: for any `T: Unpin`, `Pin::into_inner` and
`Pin::get_mut` hand the value straight back to safe code, and an assignment or
`mem::swap` through that `&mut T` relocates it. A struct that stores a pointer,
cursor, or intrusive link into its own storage keeps a stale address after such
a move, and the next dereference reads freed or unrelated memory with no
diagnostic anywhere. `PhantomPinned` is what makes a pin binding: it removes the
automatic `Unpin` impl, so once the value is pinned it can only be reached
through `Pin<&mut Self>`. Both halves are required — the opt-out and the pinned
receiver — or the address invariant rests on callers happening not to do the
safe thing that breaks it.

## Bad

```rust
struct StreamParser {
    buffer: Vec<u8>,
    cursor: *const u8, // points into `buffer`
}

impl StreamParser {
    fn new(buffer: Vec<u8>) -> Pin<Box<Self>> {
        let cursor = buffer.as_ptr();
        Box::pin(StreamParser { buffer, cursor })
    }

    // `&mut Self`, so the pin is decorative
    fn advance(&mut self) { self.cursor = unsafe { self.cursor.add(1) }; }
}

let pinned = StreamParser::new(vec![1, 2, 3]);
// No `PhantomPinned`, so `StreamParser: Unpin`: safe code walks back out.
let mut escaped = *Pin::into_inner(pinned);
escaped.advance(); // `cursor` now points into the old allocation
```

## Good

```rust
use std::marker::PhantomPinned;
use std::pin::Pin;
use std::ptr;

/// A byte cursor whose `anchor` records where the value sat when it was
/// pinned; the field is meaningful only while the value stays there.
struct Anchored {
    data: [u8; 4],
    cursor: usize,
    anchor: *const u8,
    // Opts the type out of `Unpin`, so the pin cannot be undone.
    _pin: PhantomPinned,
}

impl Anchored {
    fn new(data: [u8; 4]) -> Pin<Box<Self>> {
        let mut boxed = Box::pin(Anchored {
            data,
            cursor: 0,
            anchor: ptr::null(),
            _pin: PhantomPinned,
        });
        // Read the address only after the value reached its final slot.
        let anchor = boxed.data.as_ptr();
        // SAFETY: `anchor` and `cursor` are not structurally pinned; writing
        // them moves nothing out of the pinned allocation.
        unsafe {
            Pin::as_mut(&mut boxed).get_unchecked_mut().anchor = anchor;
        }
        boxed
    }

    /// Holds while the value still occupies the address it was pinned at.
    fn is_anchored(&self) -> bool {
        ptr::eq(self.anchor, self.data.as_ptr())
    }

    /// Mutation is reachable only through `Pin<&mut Self>`; no `&mut Self`
    /// escapes for safe code to `swap` or `replace`.
    fn next_byte(self: Pin<&mut Self>) -> Option<u8> {
        // SAFETY: only `cursor` changes, and no pinned data is moved out.
        let this = unsafe { self.get_unchecked_mut() };
        let byte = *this.data.get(this.cursor)?;
        this.cursor += 1;
        Some(byte)
    }
}

/// The same layout without the opt-out: `Loose` is `Unpin`.
struct Loose {
    data: [u8; 4],
    anchor: *const u8,
}

impl Loose {
    fn new(data: [u8; 4]) -> Pin<Box<Self>> {
        let mut boxed = Box::pin(Loose { data, anchor: ptr::null() });
        let anchor = boxed.data.as_ptr();
        boxed.anchor = anchor;
        boxed
    }

    fn is_anchored(&self) -> bool {
        ptr::eq(self.anchor, self.data.as_ptr())
    }
}

fn main() {
    let mut parser = Anchored::new([10, 20, 30, 40]);
    assert!(parser.is_anchored());
    assert_eq!(parser.as_mut().next_byte(), Some(10));
    assert_eq!(parser.as_mut().next_byte(), Some(20));
    assert!(
        parser.is_anchored(),
        "a !Unpin value stays where it was pinned"
    );
    // `*Pin::into_inner(parser)` does not compile: `Anchored` is not `Unpin`.

    let loose = Loose::new([10, 20, 30, 40]);
    assert!(loose.is_anchored());
    let escaped = *Pin::into_inner(loose);
    assert!(
        !escaped.is_anchored(),
        "without the opt-out, safe code moves the value back out of the Pin"
    );
}
```

## Pin Invariant Pitfalls

- The `PhantomPinned` field is the entire opt-out. Writing `impl Unpin for T {}`
  by hand re-opens the hole; the impl is safe code, but it hands safe callers a
  way to move a value whose invariant forbids moving, which makes the
  surrounding API unsound.
- Record the address *after* pinning, never before. `Box::pin` and
  `std::pin::pin!` move the value into its final slot, so a pointer captured
  from the pre-pin temporary is stale the moment it is stored.
- Every mutator takes `self: Pin<&mut Self>`. One accessor that yields
  `&mut Self` is enough for `mem::swap` or `mem::replace` to relocate the value
  without a single `unsafe` block at the call site.
- Decide per field whether it is structurally pinned, and say so in the
  `// SAFETY:` comment on `get_unchecked_mut`: the proof is that the operation
  touches only non-pinned fields and moves nothing out.
- `Drop::drop` receives `&mut self` even for a `!Unpin` type. A destructor that
  moves the value out of that reference violates the pin guarantee that the
  memory stays valid and unmoved until drop completes.
- Pinning does not imply the heap. `std::pin::pin!` anchors a value to the
  current stack frame, which is sufficient when the address only has to survive
  the enclosing scope.

## See Also

- [unsafe-sound-abstractions](unsafe-sound-abstractions.md) - a missing `PhantomPinned` makes a safe API reach undefined behavior
- [unsafe-safety-comment](unsafe-safety-comment.md) - every `get_unchecked_mut` needs its local proof written down
- [type-phantom-marker](type-phantom-marker.md) - zero-sized marker fields that change what a type permits
- [async-cancel-safety](async-cancel-safety.md) - futures are the most common pinned self-referential values
