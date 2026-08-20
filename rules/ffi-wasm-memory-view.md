# ffi-wasm-memory-view

> Treat a host view into WebAssembly linear memory as invalidated by any call that can allocate

## Why It Matters

A JavaScript `Uint8Array` over `wasm.memory.buffer` is a window onto the
module's linear memory at one moment. Any call back into the module can grow
that memory, and growth detaches the old buffer: the view still exists, reads
return zeros or throw, and writes land nowhere. The bug appears only when an
allocation happens to cross a page boundary, so it survives every small test
and fails on the large input. Rust's borrow checker models this exactly —
`&[u8]` into a `Vec` cannot outlive a `push` — but across the boundary nothing
enforces it.

## Bad

```javascript
const view = new Uint8Array(wasm.memory.buffer);   // captured once
const ptr = wasm.allocate(len);                    // may grow memory
view.set(bytes, ptr);                              // writes into a detached buffer
```

## Good

```rust
/// Stands in for the module's linear memory. The Rust borrow checker enforces
/// here what the host boundary cannot: a view cannot survive a growth.
pub struct Linear {
    bytes: Vec<u8>,
}

impl Linear {
    pub fn new(initial: usize) -> Self {
        Self { bytes: vec![0; initial] }
    }

    /// Re-acquired after every call that can allocate.
    pub fn view(&self) -> &[u8] {
        &self.bytes
    }

    pub fn allocate(&mut self, extra: usize) -> usize {
        let offset = self.bytes.len();
        self.bytes.resize(offset + extra, 0);
        offset
    }

    pub fn write(&mut self, offset: usize, data: &[u8]) {
        self.bytes[offset..offset + data.len()].copy_from_slice(data);
    }
}

fn main() {
    let mut memory = Linear::new(4);
    assert_eq!(memory.view().len(), 4);

    // Allocation may grow memory, so the earlier view is gone; take a new one.
    let offset = memory.allocate(4);
    memory.write(offset, b"data");

    let view = memory.view();
    assert_eq!(view.len(), 8);
    assert_eq!(&view[offset..], b"data");
}
```

## Linear Memory Handling Rules

- Re-read `memory.buffer` after every call into the module, or wrap access in a
  helper that always does.
- Copy bytes out promptly rather than holding a long-lived view across host
  logic.
- Free what the module allocated, in a `finally` or equivalent, so an exception
  on the host side does not leak linear memory.
- Pass ownership explicitly — pointer plus length, with a documented owner —
  rather than sharing a view in both directions.
- Prefer generated bindings that already handle this over hand-written glue.
- WASM's `i32`/`i64` integer types carry no signedness of their own — the same
  bits mean different values depending on which operation reads them, exactly
  like C. A host passing `-1` into an export typed `u32` on the Rust side
  receives `u32::MAX`, not an error.
- Linear memory can grow but never shrinks back down; once an instance's peak
  payload has grown it, that memory stays reserved for the instance's whole
  lifetime. This is a distinct fact from view invalidation on grow — it is
  about total memory pinned, not about a stale view.

## See Also

- [ffi-native-escape-hatch](ffi-native-escape-hatch.md) - crossing the boundary deliberately
- [ffi-logic-in-core](ffi-logic-in-core.md) - keep the logic behind the boundary
- [mem-zero-copy](mem-zero-copy.md) - when a copy out is cheaper than a shared view
- [unsafe-sound-abstractions](unsafe-sound-abstractions.md) - a view that can dangle is not a safe API
