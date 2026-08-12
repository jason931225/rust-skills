# ffi-logic-in-core

> Keep business logic in a safe core crate; limit the `*-ffi` crate to translating pointers and status codes

## Why It Matters

C ABI types (`*mut u8`, lengths, integer status codes) do not compose with idiomatic Rust ownership. Once `Message` itself stores a raw pointer, every Rust caller inherits FFI unsafety. The Microsoft Pragmatic Rust Guidelines split the work: a safe crate owns the data model and the fallible operations; the FFI crate copies bytes across the boundary and returns a `u8`. The extra types are cheaper than infecting the core crate with `#[repr(C)]` layouts.

## Bad

```rust
#[repr(C)]
pub struct Message {
    pub destination: [u8; 8],
    pub data_ptr: *mut u8,
    pub data_len: usize,
    pub data_cap: usize,
}

impl Message {
    pub fn transmit(&self) -> Result<(), &'static str> {
        if self.data_ptr.is_null() && self.data_len != 0 {
            return Err("null payload");
        }
        Ok(())
    }
}
```

## Good

```rust
pub struct Message {
    destination: [u8; 8],
    data: Vec<u8>,
}

impl Message {
    pub fn new(destination: [u8; 8], data: Vec<u8>) -> Self {
        Self { destination, data }
    }

    pub fn transmit(&self) -> Result<(), &'static str> {
        let _ = self.destination;
        if self.data.is_empty() {
            return Err("empty payload");
        }
        Ok(())
    }
}

/// FFI shim: copy the C buffers, then call the safe API.
pub unsafe fn transmit_message(
    destination: *const [u8; 8],
    data: *const u8,
    data_len: usize,
) -> u8 {
    if destination.is_null() || (data.is_null() && data_len != 0) {
        return 1;
    }
    // SAFETY: caller promised `destination` is valid and `data` is valid for `data_len`.
    let dest = unsafe { *destination };
    let bytes = unsafe { std::slice::from_raw_parts(data, data_len) }.to_vec();
    match Message::new(dest, bytes).transmit() {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

fn main() {
    let dest = [0u8; 8];
    let payload = b"ping";
    let rc = unsafe { transmit_message(&dest, payload.as_ptr(), payload.len()) };
    assert_eq!(rc, 0);
}
```

## See Also

- [ffi-sys-vs-ffi-name](ffi-sys-vs-ffi-name.md) - name the translation crate `*-ffi`, not the core crate
- [type-repr-transparent](type-repr-transparent.md) - wrap a single FFI integer in the shim, not in the domain type
- [unsafe-minimize-scope](unsafe-minimize-scope.md) - the only `unsafe` should be the pointer copies at the edge
- [unsafe-safety-comment](unsafe-safety-comment.md) - document the C-side contract next to the shim
- [ffi-native-escape-hatch](ffi-native-escape-hatch.md) - from_native/into_native stay on the wrapper
- [ffi-sys-crate-builds](ffi-sys-crate-builds.md) - hermetic -sys builds, no host-tool surprise
