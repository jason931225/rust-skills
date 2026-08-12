# ffi-sys-vs-ffi-name

> Name import wrappers `*-sys` and export shims `*-ffi`

## Why It Matters

The crate name is the first signal of which side of the ABI you are on. `foo-sys` means "thin bindings *to* an existing C library"; `foo-ffi` means "C functions *from* this Rust library." Mixing them sends reviewers to the wrong `extern` block. The Microsoft Pragmatic Rust Guidelines keep that split so workspace layouts stay predictable next to `foo` (the safe core).

## Bad

```toml
# Cargo.toml of a crate that *exports* C functions for a host app to load
[package]
name = "audio-sys"
version = "0.1.0"
edition = "2024"
```

## Good

```toml
# Bindings that call into libz
[package]
name = "z-sys"
version = "0.1.0"
edition = "2024"

# Separate crate: C ABI exported by the Rust implementation
# [package]
# name = "audio-ffi"
```

```rust
pub fn crate_role() -> &'static str {
    "sys crates import; ffi crates export"
}

fn main() {
    let _ = crate_role();
}
```

## See Also

- [ffi-logic-in-core](ffi-logic-in-core.md) - the `*-ffi` crate only translates; logic lives in `foo`
- [name-crate-no-rs](name-crate-no-rs.md) - `*-sys` / `*-ffi` are role suffixes, not a language tag like `-rs`
- [proj-lib-main-split](proj-lib-main-split.md) - keep the safe crate a normal `lib.rs`, not an FFI dump
