# proj-no-glob-reexport

> Re-export public items by name; do not `pub use foo::*` across modules or crates

## Why It Matters

A glob re-export makes every new `pub` item in the source module part of your crate's API without a reviewable line in the diff. Downstream code then depends on names you never meant to promise. The Microsoft Pragmatic Rust Guidelines allow a glob only when a `cfg`-split HAL forwards an entire platform module, where the wildcard is the point.

## Bad

```rust
mod accounts {
    pub struct Account;
    pub struct AccountId;
    pub fn secret_helper() {}
}

// Future `pub` items in `accounts` leak automatically.
pub use accounts::*;
```

## Good

```rust
mod accounts {
    pub struct Account;
    pub struct AccountId;
    pub fn secret_helper() {}
}

pub use accounts::{Account, AccountId};

#[cfg(target_os = "linux")]
mod linux {
    pub fn current_os() -> &'static str {
        "linux"
    }
}

#[cfg(not(target_os = "linux"))]
mod other {
    pub fn current_os() -> &'static str {
        "other"
    }
}

#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(not(target_os = "linux"))]
pub use other::*;

fn main() {
    let _ = Account;
    let _ = current_os();
}
```

## See Also

- [proj-pub-use-reexport](proj-pub-use-reexport.md) - named `pub use` is the supported way to flatten a public API
- [proj-prelude-module](proj-prelude-module.md) - a prelude is still a curated list, not a glob of the crate
- [doc-all-public](doc-all-public.md) - named re-exports are what rustdoc can document
