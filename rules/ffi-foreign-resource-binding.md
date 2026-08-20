# ffi-foreign-resource-binding

> Return a foreign pointer to the allocator that produced it, and wrap distinct foreign handle kinds in their own opaque types so they cannot be swapped

## Why It Matters

Calling into a C library hands Rust two obligations the compiler cannot check:
which allocator a pointer belongs to, and which handle type it actually is. A
buffer a C function allocated with its own allocator must be freed with that
library's matching free function — passing it to Rust's global allocator, or
to a different library's free function, is undefined behavior even though the
call itself type-checks. And when a C API exposes several distinct object
kinds as `void*`, nothing stops a caller from passing a `Device*` where a
`Context*` is expected; the C compiler already lost that distinction, and a
thin `*mut c_void` wrapper in Rust throws it away a second time. Both
failures are invisible until the wrong code path runs.

## Handle And Allocator Rules

- Track which allocator produced every foreign pointer that crosses into
  Rust, and free it only through that allocator's matching function — never
  assume it can be dropped, `free`'d by libc, or handed to Rust's allocator
  interchangeably.
- Prefer caller-supplied buffers for large or frequent allocations so the
  caller's allocator (stack, arena, or custom heap) stays in control; reserve
  callee-owned `new`/`free` pairs for opaque types or sizes only the callee
  knows.
- Wrap each distinct foreign handle kind in its own `#[repr(transparent)]`,
  `#[non_exhaustive]` newtype around `*mut c_void` (or the real header type)
  instead of passing `*mut c_void` directly, so the type checker rejects a
  `Device` handle passed where a `Context` handle belongs.
- Make the wrapper's constructor private to the module that receives it from
  the foreign API, so nothing outside that module can synthesize one from an
  arbitrary pointer.
- Where the foreign documentation states a lifetime dependency between two
  handles (a `Device` must not outlive the `Context` that created it), encode
  it as a Rust borrow — hold the dependent handle behind a lifetime tied to
  the owning handle — instead of two independently owned pointers a caller
  could free out of order.
- Keep the memory a caller hands in (bytes the caller allocated and owns) and
  memory this side allocates to hand back (a `Box`, a `String`) on separate,
  explicitly paired allocate/free functions; the two are different
  allocations even when both cross the same boundary, and freeing one
  through the other's deallocator is undefined behavior.
- When implementing a custom `alloc`/`dealloc` pair over `std::alloc::Layout`,
  never pass a zero-sized `Layout` to `alloc` — that is undefined behavior —
  and always `dealloc` with the exact same size and alignment the matching
  `alloc` call used.

## Bad

```rust
use std::os::raw::c_void;

extern "C" {
    fn device_open() -> *mut c_void;
    fn context_open() -> *mut c_void;
    fn context_close(ctx: *mut c_void);
}

// Both handle kinds are the same Rust type, so nothing stops a caller from
// closing a device handle with the context-closing function, or vice versa —
// the C API's distinction between object kinds is gone the moment both
// become `*mut c_void`.
fn close_context(ctx: *mut c_void) {
    unsafe { context_close(ctx) }
}
```

## Good

```rust
use std::marker::PhantomData;
use std::os::raw::c_void;

/// Wraps the opaque device handle. The private constructor means only this
/// module can produce one, and its distinct type means it cannot be passed
/// anywhere a `Context` is expected, even though both are `*mut c_void`
/// underneath.
#[repr(transparent)]
#[non_exhaustive]
pub struct Device(*mut c_void);

/// The owning handle. `Device` values borrowed from a `Context` (see
/// `Context::open_device`) carry its lifetime, so the borrow checker rejects
/// a program that would let the `Context` close before its `Device`s do.
#[repr(transparent)]
#[non_exhaustive]
pub struct Context(*mut c_void);

impl Context {
    fn from_raw(ptr: *mut c_void) -> Self {
        Self(ptr)
    }

    /// Returns a handle borrowed from `self`; the lifetime prevents it from
    /// outliving the `Context` that owns it, matching the foreign library's
    /// documented "device must not outlive context" rule.
    pub fn open_device<'ctx>(&'ctx self) -> BorrowedDevice<'ctx> {
        BorrowedDevice { device: Device(self.0), _owner: PhantomData }
    }
}

pub struct BorrowedDevice<'ctx> {
    device: Device,
    _owner: PhantomData<&'ctx Context>,
}

impl BorrowedDevice<'_> {
    pub fn handle(&self) -> &Device {
        &self.device
    }
}

fn main() {
    // Standing in for the real `context_open()` FFI call.
    let raw = std::ptr::NonNull::dangling().as_ptr();
    let ctx = Context::from_raw(raw);
    let device = ctx.open_device();
    // `device.handle()` has type `&Device`, not `&Context` — passing it to a
    // function that expects a `Context` is a type error, not a runtime bug.
    assert_eq!(device.handle().0, raw);

    // `device` cannot outlive `ctx`: dropping `ctx` and then using `device`
    // afterward fails to borrow-check, which is exactly the "device must not
    // outlive context" contract the foreign header documents in prose.
    drop(device);
    drop(ctx);
}
```

## Handle Misuse Test Cases

- a `Device` handle passed to a function parameter typed `&Context` fails to
  compile, proving the two opaque wrappers cannot be swapped;
- a `BorrowedDevice` used after its owning `Context` is dropped fails to
  borrow-check, matching the foreign library's lifetime dependency;
- nothing outside the module that defines `Device`/`Context` can construct
  one directly from a raw pointer — the constructors are not public;
- an audit of every foreign `free`/`close` call confirms it is paired with
  the allocator that produced the pointer it is given, not a generic Rust
  drop or a different library's free function.

## See Also

- [ffi-opaque-handle-lifecycle](ffi-opaque-handle-lifecycle.md) - the matching discipline for handles Rust exports to C, rather than imports from it
- [type-repr-transparent](type-repr-transparent.md) - the ABI guarantee the wrapper newtypes rely on
- [api-non-exhaustive](api-non-exhaustive.md) - keeping the wrapper's constructor out of the public contract
- [ffi-status-to-result](ffi-status-to-result.md) - converting the foreign call's own failure signal at the same boundary
- [own-lifetime-elision](own-lifetime-elision.md) - the ordinary Rust borrow this rule reuses to encode a foreign lifetime rule
