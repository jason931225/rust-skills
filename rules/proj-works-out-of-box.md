# proj-works-out-of-box

> Default features must `cargo build` on tier-1 targets with only the Rust toolchain — no extra packages, env vars, or generated-at-install steps

## Why It Matters

An application with two hundred crates will not survive a leaf that needs `protoc`, `openssl-src` plus Perl, or `SOME_SDK_ROOT` on every laptop. "it compiled on my machine" is a defect: if the crate is not *about* a platform, its default feature set builds on every tier-1 target with `cargo` and `cc`. Generate bindings and data files before you publish; gate optional native extras behind additive features. CI that runs `cargo check --workspace` on linux/mac/windows is the mechanical check.

## Bad

```toml
# Cargo.toml
[package]
name = "inventory"
edition = "2024"

[dependencies]
# Always on: every consumer must have the native SDK installed.
device-sdk-sys = "1"
```

```rust
fn main() {
    // build.rs equivalent: refuse to compile unless an SDK path is exported
    let _ = std::env::var("DEVICE_SDK_ROOT").expect("set DEVICE_SDK_ROOT");
}
```

If support for a tier-1 target is temporarily incomplete, keep the platform
edge behind an internal HAL and provide a dummy implementation that compiles
and returns an explicit unsupported error. That preserves an extension point
without pretending the hardware works. Use `cfg(target_os = ...)` for genuine
platform selection and additive opt-in features for optional SDK capability.

## Good

```toml
# Cargo.toml
[package]
name = "inventory"
edition = "2024"

[features]
default = []
hardware = ["dep:device-sdk-sys"]

[dependencies]
device-sdk-sys = { version = "1", optional = true }
```

```rust
pub fn sku_count(rows: &[(u32, u32)]) -> u32 {
    rows.iter().map(|(_, n)| n).sum()
}

fn main() {
    assert_eq!(sku_count(&[(1, 2), (2, 3)]), 5);
}
```

## One Cfg-Gated Layer, Platform-Neutral Everything Above

The HAL above is a fallback for a target you cannot fully support. The same
shape is worth adopting deliberately for targets you *do* support, because the
alternative is `#[cfg]` scattered through business logic — and every scattered
branch is a place the code compiles on the maintainer's machine and nowhere
else.

Give the platform difference one module with one cfg split, expose it through
a trait or a plain function signature that names no platform, and keep
everything above it target-neutral:

```rust
// One boundary. Everything above `Platform` is compiled identically on every
// target, so it can also be exercised without a real platform behind it.
pub trait Platform {
    fn hostname(&self) -> String;
}

pub struct Report {
    pub line: String,
}

// Ordinary logic: no cfg, no platform types, testable anywhere.
pub fn build_report(platform: &impl Platform, load: u32) -> Report {
    Report { line: format!("{} load={load}", platform.hostname()) }
}

pub struct Fake(pub &'static str);
impl Platform for Fake {
    fn hostname(&self) -> String { self.0.to_string() }
}

fn main() {
    let report = build_report(&Fake("build-box"), 3);
    assert_eq!(report.line, "build-box load=3");
}
```

The payoff is that the tests for `build_report` are not platform tests. They
compile and run on every target, including the one the developer happens to be
on, and a port becomes one new implementation of `Platform` rather than an
audit of every `#[cfg]` in the crate.

Two things keep the boundary honest: the trait's signatures must not leak a
platform-specific type, or the split has only moved; and each implementation
should be a thin translation, since logic inside a cfg-gated module is logic
that only one target's CI ever runs.

## See Also

- [proj-feature-additive](proj-feature-additive.md) - optional native bits add items; they never remove the default build
- [ffi-sys-crate-builds](ffi-sys-crate-builds.md) - when you *are* the `-sys` crate, vendor or probe instead of requiring host tools
- [proj-build-rs-minimal](proj-build-rs-minimal.md) - no network and no surprise env vars in `build.rs`
