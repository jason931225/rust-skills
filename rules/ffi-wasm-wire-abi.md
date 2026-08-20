# ffi-wasm-wire-abi

> Export a compound value across a numeric-only WebAssembly ABI as an explicit `(ptr, len)` pair, never as a pointer to a Rust type's private in-memory layout

## Why It Matters

A WebAssembly module's exported functions can only take and return numeric
types — `i32`, `i64`, `f32`, `f64` — so a `String` or `Vec<T>` cannot cross
the boundary as itself. The tempting shortcut is to return a raw pointer to
the Rust value and have the host read its fields directly, because `String`
and `Vec<T>` happen to be laid out as a pointer and a length (and `String`
adds a capacity). That layout is not part of any stability guarantee —
`#[repr(Rust)]` promises nothing about field order or size across compiler
versions — so host-side code that reads it as "two `u32` words at these
offsets" is depending on an implementation detail that happens to work today.
The contract-honest version is to decide the wire format explicitly: export
`(ptr, len)` as two plain integers, and have the host copy the bytes out
through the pointer before anything on the Rust side can move or free them.

## Wire ABI Requirements

- Export compound values across the WASM boundary as explicit numeric pairs
  — `(ptr: u32, len: u32)` for a string or byte buffer — never as a raw
  pointer to a `String`, `Vec<T>`, or other type whose field layout is not
  part of a documented ABI.
- Have the host copy bytes out of linear memory using the returned `(ptr,
  len)` before making any further call into the module; a call that can
  allocate invalidates any cached view into memory, independent of this
  rule's wire-format point.
- Keep input allocation and output allocation on separate, explicitly paired
  alloc/free functions. Bytes the host copied in via a module-exported
  `alloc` free through a matching `free(ptr, len)`; a `String` or `Box<T>`
  the module produced frees through its own `Box::into_raw`/`from_raw` pair.
  Do not mix the two allocators.
- Do not hand-write a second implementation of this marshalling if a binding
  generator (`wasm-bindgen` or similar) is already in use for the same
  boundary — the generator's glue is exactly this out-pointer contract,
  generated correctly and kept in sync with the toolchain.
- Mark an exported function `unsafe` (or route it through a generated binder
  that upholds the contract) whenever it reconstructs a `&str`/`String` from
  host-supplied bytes without validating them — the host is an untrusted
  caller from the module's point of view, the same as any other FFI boundary.

## Bad

```rust
// The host is expected to read this pointer as two u32 words — String's
// pointer and length — reconstructed by reading raw bytes at fixed offsets
// into the struct. Nothing about #[repr(Rust)] promises that layout, size,
// or field order stay the same across a compiler upgrade.
#[unsafe(no_mangle)]
pub extern "C" fn greet() -> *mut String {
    Box::into_raw(Box::new(String::from("hello")))
}
```

## Good

```rust
use std::alloc::{alloc, dealloc, Layout};

/// Returns an explicit (ptr, len) pair as two plain integers — the numeric
/// wire format every WASM host ABI actually supports — rather than a
/// pointer into a Rust type's private layout. On a real wasm32 target both
/// fields fit natively in `u32`; this example uses `u64` because it runs on
/// a 64-bit host, where a real heap pointer does not fit in 32 bits at all —
/// packing it into one would truncate the address, not model the ABI.
#[unsafe(no_mangle)]
pub extern "C" fn greeting_ptr_len() -> (u64, u64) {
    let message = "hello";
    let bytes = message.as_bytes();
    let layout = Layout::array::<u8>(bytes.len()).expect("layout for greeting bytes");
    // SAFETY: `layout` has non-zero size (the literal is non-empty), and the
    // returned pointer's ownership transfers to the host, which must free it
    // with `free_bytes` using this exact layout.
    let ptr = unsafe { alloc(layout) };
    assert!(!ptr.is_null(), "allocation failed");
    // SAFETY: `ptr` was just allocated with room for `bytes.len()` bytes.
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len()) };
    (ptr as u64, bytes.len() as u64)
}

/// The matching free: same allocator, same size and alignment as the
/// `alloc` call that produced `ptr`.
#[unsafe(no_mangle)]
pub extern "C" fn free_bytes(ptr: *mut u8, len: usize) {
    if len == 0 {
        return; // a zero-sized Layout is UB to pass to dealloc
    }
    let layout = Layout::array::<u8>(len).expect("layout for freed bytes");
    // SAFETY: `ptr`/`len` are exactly what `greeting_ptr_len` returned, and
    // this is the only place that frees them.
    unsafe { dealloc(ptr, layout) };
}

fn main() {
    let (ptr, len) = greeting_ptr_len();
    let ptr = ptr as *mut u8;
    let len = len as usize;

    // Stands in for the host copying bytes out of linear memory using the
    // numeric (ptr, len) pair, before calling back into the module again.
    let copied = unsafe { std::slice::from_raw_parts(ptr, len) }.to_vec();
    assert_eq!(copied, b"hello");

    free_bytes(ptr, len);
}
```

## Marshalling Cases To Test

- the returned `(ptr, len)` pair, read as two plain integers, recovers
  exactly the bytes the module produced;
- freeing with a length that does not match the original allocation is
  caught by a debug allocator or Miri as a Layout mismatch;
- an input buffer allocated through the module's `alloc` export and an
  output buffer produced via `Box::into_raw` are freed through their own
  distinct paired functions, never through each other's;
- a zero-length free is a no-op rather than a call into `dealloc` with a
  zero-sized `Layout`.

## See Also

- [ffi-wasm-memory-view](ffi-wasm-memory-view.md) - the view-invalidation half of working across this same boundary
- [ffi-foreign-resource-binding](ffi-foreign-resource-binding.md) - the general allocator-pairing discipline this rule specializes for WASM
- [unsafe-sound-abstractions](unsafe-sound-abstractions.md) - marking the reconstruction of host-supplied bytes `unsafe`
- [type-repr-transparent](type-repr-transparent.md) - the ABI guarantee `#[repr(Rust)]` does *not* give you here
- [ffi-c-bitflag-enum](ffi-c-bitflag-enum.md) - a sibling case of picking an explicit wire representation instead of relying on Rust's internal layout
