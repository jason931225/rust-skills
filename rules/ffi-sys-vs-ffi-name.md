# ffi-sys-vs-ffi-name

> Name import wrappers `*-sys` and export shims `*-ffi`

## Why It Matters

The crate name is the first signal of which side of the ABI you are on. `foo-sys` means "thin bindings *to* an existing C library"; `foo-ffi` means "C functions *from* this Rust library." Mixing them sends reviewers to the wrong `extern` block. Keep that split so workspace layouts stay predictable next to `foo` (the safe core).

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

The ecosystem has established spelling variants. An underscore may replace a
hyphen in identifiers, and an abandoned binding crate may force a successor
name such as `foo-sys2`. Preserve the import/export meaning even when registry
history prevents the ideal spelling.

## Depending On Raw Bindings Or A Safe Wrapper

Naming your own crates is one question; which of the two to *depend on* is
another, and the answer is not automatic.

- **The `-sys` crate** gives you the C surface directly. Every call needs an
  `unsafe` block, a safety comment, and its own status-code handling, and any
  resource it returns has to be released by whatever the library says frees it.
  In exchange there is no wrapper to learn, no wrapper to keep current, and
  nothing between you and the semantics the C documentation describes.
- **The safe wrapper** pays those costs once and hands back `Result` and RAII.
  It is the default worth reaching for, and the questions to ask are whether it
  covers the calls you need, and whether it is maintained on the same cadence
  as the C library — a wrapper pinned to an older release is a second
  compatibility floor you now have.

Take one or the other for a given library. Both in one graph means two paths to
the same native handles, with the wrapper's invariants applying to only half of
them; if you need an escape hatch, use the one the wrapper exposes rather than
adding a parallel dependency on its `-sys` crate.

## See Also

- [ffi-logic-in-core](ffi-logic-in-core.md) - the `*-ffi` crate only translates; logic lives in `foo`
- [name-crate-no-rs](name-crate-no-rs.md) - `*-sys` / `*-ffi` are role suffixes, not a language tag like `-rs`
- [proj-lib-main-split](proj-lib-main-split.md) - keep the safe crate a normal `lib.rs`, not an FFI dump
- [ffi-sys-crate-builds](ffi-sys-crate-builds.md) - hermetic builds for the import crate
- [ffi-native-escape-hatch](ffi-native-escape-hatch.md) - native handle conversions on the wrapper
