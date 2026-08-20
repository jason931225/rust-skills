# ffi-dll-portable-state

> Share only portable, repr-stable values across Rust dynamic libraries; keep allocations, statics, TLS, and TypeId local to the DLL that created them

## Why It Matters

The compiler treats each Rust dylib as its own program, with its own `static`s,
thread-locals, `TypeId` table, and allocator. `#[repr(Rust)]` layout may differ
between compilations, so handing a `String`, Tokio handle, or default-repr
struct to another DLL can free the wrong heap, use the wrong statics, or make a
meaningless `TypeId` comparison. FFI safety alone is insufficient for
cross-image ownership: portable values also need a defined layout and no
dependency on image-local state. Allow only portable state across that edge,
both when loading and publishing plugins.

## What Counts As Portable

A portable value has a defined layout (`#[repr(C)]` or equivalent) and
satisfies every constraint below:

- It has no interaction with any `static` or thread-local.
- It has no interaction with any `TypeId`.
- It contains no value, pointer, or reference to non-portable data.
- Both images agree on the value definition and ABI version. Export a
  versioned entry point or `#[repr(C)]` API table and reject a mismatch before
  passing values.

A pointer *into* portable bytes that still live in a non-portable owner is
allowed: a `*const u8` / length pair into a `Vec<u8>` that the allocating DLL
keeps is portable; the `Vec` itself is not. *Interaction* is any computational
relationship, including how the bits are interpreted. Passing a `u128` is fine;
transmuting a `TypeId` into that `u128` is not.

## Bad

```rust
pub struct Meter {
    label: String,
}

impl Meter {
    pub fn reading(&self) -> u64 {
        // Resolved in *this* crate. Across a DLL boundary the bytes
        // behind `self` were laid out by the other image, but this
        // method still uses this image's statics and TypeId table.
        self.label.len() as u64
    }
}

/// # Safety
///
/// Demonstration only: `meter` must point to a live `Meter`, but the type is
/// still not portable across an independently compiled library boundary.
pub unsafe fn drive(meter: *const Meter) -> u64 {
    // SAFETY: demonstration only — the type is not portable.
    unsafe { (*meter).reading() }
}

fn main() {
    let meter = Meter {
        label: String::from("pump"),
    };
    let n = unsafe { drive(&meter) };
    assert_eq!(n, 4);
}
```

## Good

```rust
#[repr(C)]
pub struct Sample {
    pub millis: u32,
    pub value: i32,
}

#[repr(C)]
pub struct ByteView {
    pub ptr: *const u8,
    pub len: usize,
}

pub const PLUGIN_ABI_VERSION: u32 = 1;

#[repr(C)]
pub struct PluginApi {
    pub abi_version: u32,
    pub fill_sample: FillSample,
}

/// # Safety
///
/// `view.ptr` must be valid for `view.len` bytes until this function
/// returns. The caller keeps the allocation and the matching allocator;
/// this side must copy if it needs the bytes later.
pub unsafe extern "C" fn checksum(view: ByteView) -> u32 {
    if view.ptr.is_null() {
        return 0;
    }
    // SAFETY: caller owns the buffer for the duration of the call.
    let bytes = unsafe { std::slice::from_raw_parts(view.ptr, view.len) };
    bytes.iter().fold(0u32, |acc, b| acc.wrapping_add(u32::from(*b)))
}

pub type FillSample = unsafe extern "C" fn(*mut Sample) -> i32;

unsafe extern "C" fn fill_zero(out: *mut Sample) -> i32 {
    if out.is_null() {
        return 1;
    }
    // SAFETY: non-null `out` is a live `Sample` the caller owns.
    unsafe {
        *out = Sample {
            millis: 0,
            value: 0,
        };
    }
    0
}

fn validate_plugin(api: &PluginApi) -> Result<(), &'static str> {
    if api.abi_version != PLUGIN_ABI_VERSION {
        return Err("plugin abi version mismatch");
    }
    Ok(())
}

fn main() {
    let owned = b"ok";
    let view = ByteView {
        ptr: owned.as_ptr(),
        len: owned.len(),
    };
    // SAFETY: `view` points to `owned`, which remains live for this call.
    let sum = unsafe { checksum(view) };
    assert_eq!(sum, u32::from(b'o') + u32::from(b'k'));

    let api = PluginApi {
        abi_version: PLUGIN_ABI_VERSION,
        fill_sample: fill_zero,
    };
    assert!(validate_plugin(&api).is_ok());

    let mut sample = Sample {
        millis: 9,
        value: 4,
    };
    // SAFETY: `sample` is live and uniquely borrowed for this call.
    let rc = unsafe { (api.fill_sample)(&mut sample) };
    assert_eq!(rc, 0);
    assert_eq!(sample.millis, 0);
}
```

## Cross-DLL Hazards

- **Allocator ownership.** `String`, `Vec<T>`, `Box<T>`, and anything else that frees on drop must be dropped by the DLL that allocated them. Crossing the boundary is a cross-heap free.
- **Transitive portability.** Every nested field must be portable. Wrapping a `String` in `#[repr(C)]` does not make the heap value portable.
- **Nested pointers.** A pointer or reference is portable only when it addresses portable data. Pointing at a Rust-layout node inside a C envelope is still UB.
- **Byte views need a protocol.** A slice or `ptr`+`len` view is portable for primitive bytes; document who allocated, who copies, who frees, and that both the owner and the loaded image remain live while any data or function pointer is in use.
- **Hidden method hazard.** A method call on a foreign object is compiled in *your* image. The code that runs is yours; the bytes are theirs. Prefer an `extern "C"` function pointer the originating DLL provides.
- **Libraries with process state.** Runtimes and loggers that keep static registries (`tokio`, `log`, similar) are not portable handles. Give each DLL its own instance, or talk through a C ABI you control.
- **No unwinding.** Foreign boundary functions return status codes. Catch a
  panic at the boundary only when the process uses unwind semantics, convert
  it to an error, and keep process isolation for `panic = "abort"`.
- **Failures compound.** Passing owning, default-layout, or image-state-bound
  values across libraries can cause silent data loss, corrupted state, and
  undefined behavior.

## See Also

- [ffi-logic-in-core](ffi-logic-in-core.md) - translate at the boundary; do not ship domain types as C layouts
- [ffi-native-escape-hatch](ffi-native-escape-hatch.md) - documented raw conversions stay on the wrapper
- [type-repr-transparent](type-repr-transparent.md) - a one-field ABI wrapper is layout-stable; a Rust struct is not
- [proj-avoid-statics](proj-avoid-statics.md) - process-identity state is already unsound inside one binary
- [conc-thread-local](conc-thread-local.md) - TLS is per image, not per process
- [err-catch-unwind-boundary](err-catch-unwind-boundary.md) - convert unwind failures only at an isolation edge
- [unsafe-minimize-scope](unsafe-minimize-scope.md) - the only `unsafe` at the edge should be the pointer copies
