# proj-feature-additive

> Design Cargo features to be strictly additive

## Why It Matters

Cargo unifies features across the dependency graph: if any crate in the build enables a feature, every consumer of that crate gets it. A feature that removes or changes existing behavior will break crates that depend on the baseline behavior the moment a third dependency enables it. Features must only add capability — new trait impls, additional dependencies, optional integrations — never subtract. Mutually exclusive features are an anti-pattern in the Cargo model.

## Bad

```toml
[features]
# "no_std" disables std — enabling it REMOVES behavior
no_std = []

[dependencies]
# and somewhere in lib.rs:
# #[cfg(not(feature = "no_std"))]
# use std::collections::HashMap;
```

```rust
// lib.rs — toggling off std via a feature is non-additive
#[cfg(not(feature = "no_std"))]
use std::vec::Vec;

#[cfg(feature = "no_std")]
use alloc::vec::Vec;
```

## Good

```toml
[features]
# "std" ADDS std support; no_std is the baseline
default = ["std"]
std = []

# Optional integrations — purely additive
serde = ["dep:serde"]
tokio = ["dep:tokio"]

[dependencies]
serde = { version = "1", optional = true }
tokio = { version = "1", optional = true }
```

```rust
// lib.rs — std is opt-in, no_std is the default baseline
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
use std::vec::Vec;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
```

## Rules for Additive Features

- A feature may add new items, trait impls, dependencies, or variants to an already `#[non_exhaustive]` enum—never gate away existing behavior.
- If you ship a `no_std` crate, make `std` a feature in `default`, not the other way around.
- Every feature must work in every unified combination. Do not publish mutually exclusive features, and do not use `compile_error!` as the normal backend-selection mechanism.
- A feature enables every feature it requires; callers must not need to discover and add a second feature manually.
- Do not rely on a parent crate suppressing a child dependency feature. Another graph path may enable it, and Cargo will unify it globally.
- Use `dep:` syntax (`dep:serde`) to keep optional dependency names out of the feature namespace.
- Name the capability (`serde`, `tls`, `metrics`), not a placeholder such as `extras`, `misc`, `full2`, or `unstable-stuff`.

## See Also

- [api-serde-optional](api-serde-optional.md) - gate Serialize/Deserialize behind a feature flag
- [proj-workspace-deps](proj-workspace-deps.md) - use workspace dependency inheritance
- [lint-cfg-check](lint-cfg-check.md) - catch feature-gate typos with unexpected_cfgs
- [proj-works-out-of-box](proj-works-out-of-box.md) - default features must still cargo-build everywhere
- [test-util-feature](test-util-feature.md) - test-only helpers are an additive feature
