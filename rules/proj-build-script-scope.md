# proj-build-script-scope

> A `build.rs` configures only its own package, and must decide from the target — never from what it finds on the build machine

## Why It Matters

Two things about build scripts surprise people in opposite directions. A
`build.rs` cannot configure its dependents: `cargo::rustc-cfg`, environment
variables, and link arguments it emits apply to *its own* package's
compilation, and reach a downstream crate only when the package declares
`links` and emits `cargo::metadata` the dependent reads back. Code that sets a
cfg in a `-sys` crate and expects the wrapper to see it is silently
mis-configured. In the other direction, a build script runs on the *build*
machine with full access to it, so probing for an installed library, a device
node, or a CPU feature bakes the builder's environment into the artifact — the
same source and the same feature set then produce two different binaries
depending on where they were compiled, which breaks reproducibility and
cross-compilation at once.

## What A Build Script May Decide From

- Read the target, not the host: `TARGET`, `CARGO_CFG_TARGET_OS`,
  `CARGO_CFG_TARGET_ARCH`, `CARGO_CFG_TARGET_ENV`, and the package's own
  Cargo features. These are the same on every machine building for that target.
- Do not emit `cargo::rustc-cfg` from a probe of the build machine — a
  `pkg-config` hit for an optional library, `Path::exists` on a device node, a
  CPU-feature query. Model optional capability as a Cargo feature (a build-time
  decision the caller makes) or detect it at runtime (a deployment-time fact),
  and keep the two straight.
- Emit `cargo::rerun-if-changed` for every input the script actually reads.
  Cargo fingerprints the compiled script binary, but that does not replace the
  directives: with none emitted, any change in the package retriggers the
  script, and with the wrong ones, a real input change does not.
- Expect nothing you emit to reach a dependent unless the package sets `links`
  and the dependent reads the corresponding `DEP_<NAME>_<KEY>` variables. Plan
  cross-crate configuration as an explicit `links` contract, not as an
  ambient side effect.
- Fail with a message that names the missing tool, library, or path. A bare
  `unwrap()` in a build script surfaces as "build script failed" with no
  indication of what to install.
- Where the build embeds a timestamp for traceability, honor `SOURCE_DATE_EPOCH`
  (falling back to the commit time) rather than `SystemTime::now()`, so the
  same source still produces the same bytes.

## Bad

```rust
// build.rs — decides from the machine doing the build.
fn main() {
    // Whether the builder happens to have this library installed is now
    // compiled into the artifact. The same source and features produce a
    // different binary on a different machine, and cross-compiling detects
    // the host's libraries rather than the target's.
    if std::path::Path::new("/usr/lib/libsystemd.so").exists() {
        println!("cargo::rustc-cfg=has_systemd");
    }
    // Nothing declares what this script read, so Cargo's rerun heuristics
    // have nothing to go on.
}
```

## Good

```rust
// build.rs — decides from the target and the package's own features.
fn main() {
    // The target triple is identical for everyone building this artifact.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "linux" {
        println!("cargo::rustc-cfg=uses_epoll");
    }

    // Optional capability is a feature the caller opts into, not something
    // discovered on the build machine.
    if std::env::var_os("CARGO_FEATURE_SYSTEMD").is_some() {
        println!("cargo::rustc-cfg=has_systemd");
    }

    // Declare every input actually read, so the rebuild trigger is exact.
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=src/schema.capnp");
    println!("cargo::rerun-if-env-changed=SOURCE_DATE_EPOCH");
}

fn main_documentation_only() {
    // Shown for the contract, not run: a build script's `cargo::rustc-cfg`
    // applies to this package alone. To hand a value to dependents, the
    // package sets `links = "systemd"` in Cargo.toml and emits metadata,
    // which dependents read as DEP_SYSTEMD_INCLUDE.
    println!("cargo::metadata=include=/usr/include/systemd");
}
```

## Cases To Pin In Tests

- building the same source with the same features on two machines with
  different installed libraries produces the same set of emitted cfgs;
- cross-compiling to a different target emits the target's cfgs, not the
  host's;
- touching a file the script reads triggers a rebuild, and touching an
  unrelated file in the package does not (the `rerun-if-changed` set is
  exact, in both directions);
- a missing required tool fails with a message naming it, not a bare
  "build script failed";
- a dependent that needs a value from this package's script reads it through
  a declared `links` contract, and a test confirms removing `links` breaks
  that dependent loudly rather than silently.

## See Also

- [proj-build-rs-minimal](proj-build-rs-minimal.md) - keeping the script itself small, deterministic, and idempotent
- [proj-build-target-cfg](proj-build-target-cfg.md) - reading the target rather than `cfg!`, which reflects the host
- [ffi-sys-crate-builds](ffi-sys-crate-builds.md) - the `-sys` crate whose script most often needs a `links` contract
- [proj-feature-additive](proj-feature-additive.md) - modelling optional capability as a feature instead of a probe
- [proj-reproducible-runtime](proj-reproducible-runtime.md) - why a build-machine-dependent artifact defeats a reproducible pipeline
