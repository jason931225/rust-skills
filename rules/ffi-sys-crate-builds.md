# ffi-sys-crate-builds

> Keep `-sys` crates hermetic: vendored C sources or a `pkg-config` probe, no one-off host tools on the default path

## Why It Matters

Every consumer of `foo-sys` inherits its build. A `build.rs` that shells out to `nasm`, `perl`, or a downloaded tarball fails on the next machine, in CI, and in any sandbox that has only `cc` and a linker. The Microsoft Pragmatic Rust Guidelines require a `-sys` crate to compile with the Rust toolchain plus `cc`. Vendor the upstream sources (or document a hash-pinned fetch behind a non-default feature), generate bindgen output before publish when you can, and offer static linking. `proj-build-rs-minimal` is the local script hygiene; this rule is the interop contract.

## Bad

```rust
// build.rs
fn main() {
    std::process::Command::new("perl")
        .arg("generate-bindings.pl")
        .status()
        .expect("perl must be on PATH");
}
```

## Good

```toml
# foo-sys/Cargo.toml
[package]
name = "foo-sys"
edition = "2024"

[build-dependencies]
cc = "1"
```

```
// foo-sys/build.rs
// Compile vendored C with the `cc` crate. No perl, nasm, or network fetch
// on the default path. Sources live in vendor/ next to this script.
//
//   println!("cargo::rerun-if-changed=vendor/foo.c");
//   cc::Build::new().file("vendor/foo.c").include("vendor").compile("foo");
```

```rust
pub fn linked_native_lib() -> &'static str {
    "foo"
}

fn main() {
    let _ = linked_native_lib();
}
```

## See Also

- [ffi-sys-vs-ffi-name](ffi-sys-vs-ffi-name.md) - this contract applies to `*-sys`, not to `*-ffi` export crates
- [proj-build-rs-minimal](proj-build-rs-minimal.md) - keep the script deterministic and offline
- [proj-works-out-of-box](proj-works-out-of-box.md) - the same "just `cargo build`" bar for ordinary libraries
