# ffi-native-escape-hatch

> Give native-handle wrappers `from_native` / `into_native` / `as_native` so FFI code can cross the boundary without leaking the raw type everywhere

## Why It Matters

A safe `Handle` that hides the OS value is useless the moment a caller already has a `RawFd` from C, or must pass yours into another library. The Microsoft Pragmatic Rust Guidelines ask for a documented, `unsafe` conversion pair: `from_native` states the ownership and validity rules, `into_native` / `as_native` give the integer or pointer back. Keep those methods on the wrapper; do not publish the raw type as the crate's currency (`ffi-logic-in-core`).

## Bad

```rust
pub struct Handle(i32);

impl Handle {
    pub fn new(fd: i32) -> Self {
        Self(fd)
    }
}
```

## Good

```rust
pub struct Handle(i32);

impl Handle {
    pub fn new(fd: i32) -> Self {
        Self(fd)
    }

    /// # Safety
    ///
    /// `fd` must be an open descriptor this process owns, and no other
    /// `Handle` may wrap the same number.
    pub unsafe fn from_native(fd: i32) -> Self {
        Self(fd)
    }

    pub fn into_native(self) -> i32 {
        self.0
    }

    pub fn as_native(&self) -> i32 {
        self.0
    }
}

fn main() {
    let handle = Handle::new(3);
    assert_eq!(handle.as_native(), 3);
    let raw = handle.into_native();
    let _ = unsafe { Handle::from_native(raw) };
}
```

## See Also

- [ffi-logic-in-core](ffi-logic-in-core.md) - the escape hatch lives on the wrapper, not on the domain type
- [type-repr-transparent](type-repr-transparent.md) - a newtype around the native integer stays ABI-compatible
- [unsafe-safety-comment](unsafe-safety-comment.md) - `from_native` is `unsafe`; write the `# Safety` contract
