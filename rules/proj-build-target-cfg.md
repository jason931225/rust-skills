# proj-build-target-cfg

> Write `build.rs` against the target, not the host: read `TARGET`, `HOST`, and `CARGO_CFG_TARGET_*` instead of `cfg!`

## Why It Matters

A build script is compiled for the host and executed on the host, so `cfg!(target_os = "windows")` inside `build.rs` describes the machine doing the building, never the machine that will run the artifact. In a native build the two agree, which is exactly why the mistake ships: it first appears when CI cross-compiles, as a link against a library that does not exist on the target, a wrong-width constant baked into generated code, or an object file built for the host ISA and handed to a target linker. Shelling out to `gcc` has the same shape — it resolves to the host compiler and ignores the `CC_<triple>`, `CFLAGS_<triple>`, and sysroot settings the cross toolchain already supplies.

## Bad

```rust
// build.rs — every decision below describes the build machine
fn main() {
    // `cfg!` was resolved when this script was compiled, for the host.
    if cfg!(target_os = "linux") {
        println!("cargo::rustc-link-lib=nsl");
    }
    if cfg!(target_pointer_width = "64") {
        println!("cargo::rustc-cfg=wide_handles");
    }

    // Host compiler, host ISA, host libc — linked into a target binary.
    std::process::Command::new("gcc")
        .args(["-c", "vendor/shim.c", "-o", "shim.o"])
        .status()
        .expect("gcc");
}
```

Cross-compiling this crate from x86_64 Linux to `x86_64-pc-windows-msvc` still
requests `nsl` and still produces ELF objects.

## Good

The real script reads the target from the environment and hands native
compilation to `cc`, which honours the cross toolchain:

```text
// build.rs
println!("cargo::rerun-if-changed=vendor/shim.c");

let target_os = std::env::var("CARGO_CFG_TARGET_OS")?;   // "windows", "linux", ...
let target_env = std::env::var("CARGO_CFG_TARGET_ENV")?; // "msvc", "gnu", "musl"

if target_os == "windows" {
    println!("cargo::rustc-link-lib=ws2_32");
}

// cc reads TARGET / HOST / OPT_LEVEL / CC_<triple> and picks the cross compiler.
cc::Build::new().file("vendor/shim.c").compile("shim");
```

The decision logic itself is ordinary Rust and can be exercised without Cargo:

```rust
use std::collections::HashMap;

/// The facts a build script may act on, all sourced from the environment
/// Cargo sets for the artifact's target — never from `cfg!`, which describes
/// the machine the script itself was compiled for.
#[derive(Debug, PartialEq, Eq)]
struct BuildTarget {
    triple: String,
    os: String,
    env: String,
    pointer_width: u32,
    cross_compiling: bool,
}

fn var(env: &HashMap<String, String>, key: &str) -> Result<String, String> {
    env.get(key)
        .cloned()
        .ok_or_else(|| format!("{key} is unset; this script must run under cargo"))
}

impl BuildTarget {
    fn from_env(env: &HashMap<String, String>) -> Result<Self, String> {
        let triple = var(env, "TARGET")?;
        let host = var(env, "HOST")?;
        let width = var(env, "CARGO_CFG_TARGET_POINTER_WIDTH")?;
        Ok(Self {
            cross_compiling: triple != host,
            os: var(env, "CARGO_CFG_TARGET_OS")?,
            env: var(env, "CARGO_CFG_TARGET_ENV")?,
            pointer_width: width.parse().map_err(|_| format!("bad pointer width {width}"))?,
            triple,
        })
    }

    /// The decision a build script emits as `cargo::rustc-link-lib=...`.
    fn socket_lib(&self) -> &'static str {
        match (self.os.as_str(), self.env.as_str()) {
            ("windows", _) => "ws2_32",
            ("linux", "musl") => "c", // statically linked; no separate resolver lib
            ("linux", _) => "nsl",
            _ => "c",
        }
    }
}

fn env_of(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()
}

fn main() {
    let host = "x86_64-unknown-linux-gnu";

    let musl = BuildTarget::from_env(&env_of(&[
        ("HOST", host),
        ("TARGET", "aarch64-unknown-linux-musl"),
        ("CARGO_CFG_TARGET_OS", "linux"),
        ("CARGO_CFG_TARGET_ENV", "musl"),
        ("CARGO_CFG_TARGET_POINTER_WIDTH", "64"),
    ]))
    .expect("cargo sets every variable read here");

    let windows = BuildTarget::from_env(&env_of(&[
        ("HOST", host),
        ("TARGET", "x86_64-pc-windows-msvc"),
        ("CARGO_CFG_TARGET_OS", "windows"),
        ("CARGO_CFG_TARGET_ENV", "msvc"),
        ("CARGO_CFG_TARGET_POINTER_WIDTH", "64"),
    ]))
    .expect("cargo sets every variable read here");

    // One running binary, two targets, two answers. `cfg!(target_os = ...)`
    // is a constant here and cannot produce this.
    assert_eq!(musl.socket_lib(), "c");
    assert_eq!(windows.socket_lib(), "ws2_32");
    assert!(musl.cross_compiling && windows.cross_compiling);

    // The libc flavour lives in TARGET_ENV; TARGET_OS alone cannot see it.
    assert_eq!(musl.os, "linux");
    assert_eq!(musl.env, "musl");
    assert_eq!(musl.pointer_width, 64);

    // A missing variable is a loud failure, not a silent host-shaped default.
    let outside_cargo = BuildTarget::from_env(&env_of(&[("HOST", host)]));
    assert!(outside_cargo.unwrap_err().contains("TARGET"));
}
```

## Key Points

- `cfg!` and `#[cfg]` in a build script describe the host; `CARGO_CFG_*` describes
  the target. They agree only in a native build, so local testing never catches
  the confusion.
- Cargo exposes `CARGO_CFG_TARGET_OS`, `_ARCH`, `_FAMILY`, `_ENV`, `_ENDIAN`,
  `_POINTER_WIDTH`, and `_FEATURE`, plus the full `TARGET` and `HOST` triples.
  Multi-valued cfgs arrive comma-separated, and `TARGET != HOST` is the
  cross-compiling test.
- The libc flavour is `CARGO_CFG_TARGET_ENV` (`gnu`, `musl`, `msvc`), not the OS.
  A `linux` check alone cannot tell a static musl build from a glibc one, which is
  where "cannot find -lssl" and missing-symbol failures come from.
- Route native compilation through `cc`, which reads `TARGET`, `HOST`,
  `OPT_LEVEL`, and `CC_<triple>` / `CFLAGS_<triple>` and therefore uses whatever
  cross compiler `.cargo/config.toml` or the CI image installed. A hard-coded
  `Command::new("gcc")` cannot.
- Absent variables mean the script is not running under Cargo; fail with the
  variable name rather than defaulting to a host-shaped guess.
- Declare every custom cfg the script emits in `check-cfg`, so a per-target
  `cargo::rustc-cfg` that never matches becomes a warning instead of dead code.
- Prove the wiring in CI with a cross-build matrix; a target that only ever
  builds natively has never exercised these code paths.

## See Also

- [proj-build-rs-minimal](proj-build-rs-minimal.md) - keep the same script deterministic, offline, and narrowly re-run
- [ffi-sys-crate-builds](ffi-sys-crate-builds.md) - hermetic native builds driven by `cc` rather than ambient tools
- [proj-cfg-select](proj-cfg-select.md) - select per-target items inside the crate's own source, where `cfg` does mean the target
- [lint-cfg-check](lint-cfg-check.md) - declare the cfgs a build script emits so typos surface
