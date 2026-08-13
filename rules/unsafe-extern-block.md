# unsafe-extern-block

> In Rust 2024, wrap `extern` blocks in `unsafe extern { }` and annotate each item as `safe` or `unsafe`.

## Why It Matters

Before Rust 2024, functions inside an `extern "C" { }` block were implicitly
unsafe to call even though the block carried no `unsafe` keyword. Rust 2024
requires `unsafe extern`, making the programmer's responsibility for accurate
types, pointers, and aliasing explicit. Individual items can then be marked
`safe` for callers or remain `unsafe` by default. This makes FFI boundaries
auditable while allowing reviewed wrappers to expose safe calls.

## Bad

```rust
// Rust 2021 style — compiles but forbidden in 2024 edition
extern "C" {
    fn strlen(s: *const std::ffi::c_char) -> usize;
    fn memcpy(dst: *mut u8, src: *const u8, n: usize) -> *mut u8;
    static errno: std::ffi::c_int;
}
```

## Good

```rust
// Rust 2024 style
unsafe extern "C" {
    // `strlen` is genuinely unsafe: caller must pass a null-terminated pointer.
    pub unsafe fn strlen(s: *const std::ffi::c_char) -> usize;

    // `memcpy` is unsafe: caller must ensure non-overlapping, valid regions.
    pub unsafe fn memcpy(dst: *mut u8, src: *const u8, n: usize) -> *mut u8;

    // A function that is always safe to call (hypothetical pure query).
    pub safe fn rust_version_major() -> u32;

    // Mutable foreign state is unsafe to access unless its synchronization
    // contract prevents races.
    pub unsafe static mut FOREIGN_COUNTER: std::ffi::c_int;
}

// A safe wrapper establishes the raw call's complete preconditions.
fn copy_bytes(dst: &mut [u8], src: &[u8]) {
    assert!(dst.len() >= src.len(), "destination is too short");
    // SAFETY: the slices are valid for src.len() bytes; Rust's borrowing rules
    // prevent dst from overlapping src while both references are live.
    unsafe { memcpy(dst.as_mut_ptr(), src.as_ptr(), src.len()) };
}

// Call sites for `safe` items need no unsafe block:
fn show_version() {
    println!("major: {}", rust_version_major());
}
```

## Migration from 2021

| 2021 | 2024 |
|------|------|
| `extern "C" { fn foo(); }` | `unsafe extern "C" { unsafe fn foo(); }` |
| `extern "C" { fn bar(); }` (safe to call) | `unsafe extern "C" { safe fn bar(); }` |
| `extern "C" { static X: i32; }` | `unsafe extern "C" { unsafe static X: i32; }` |

Run `cargo fix --edition` to apply the mechanical part of this migration automatically. Review each item afterward to decide whether `safe` is warranted.

## Key Points

- The `unsafe` on the block means "I assert these declarations faithfully describe the external ABI". It does not make calls to the items safe by itself.
- Marking an item `safe` is a promise: if that item is actually unsafe to call, adding `safe` is itself unsound — the compiler will not catch a wrong annotation.
- Regenerate `bindgen` output with a release that supports Rust 2024 rather
  than hand-editing generated blocks. `cbindgen` serves the opposite direction
  (Rust exports to C/C++ headers) and does not replace import-side ABI review.
- Use `std::ffi::c_*` types for C ABI declarations instead of assuming a Rust primitive has the same width. Rust 1.96 changed `c_double` to `f32` on AVR to match that platform's C ABI; a hard-coded `f64` declaration is wrong there.
- Imports using `extern "Rust"` require the same unsafe block syntax, but the
  Rust ABI has no cross-version stability guarantee. Use a stable C-compatible
  ABI for independently built components (see `unsafe-no-mangle-unsafe`).

## See Also

- [unsafe-no-mangle-unsafe](unsafe-no-mangle-unsafe.md) - mark `#[no_mangle]` as `#[unsafe(no_mangle)]` in Rust 2024
- [type-repr-transparent](type-repr-transparent.md) - use `#[repr(transparent)]` for FFI newtypes
- [ffi-logic-in-core](ffi-logic-in-core.md) - isolate raw extern declarations in the FFI crate
