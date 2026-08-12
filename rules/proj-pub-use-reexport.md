# proj-pub-use-reexport

> Give each owned item one public path; re-export a foreign type only when it is part of your contract

## Why It Matters

`pub use` lets you keep a deep internal tree and still offer `the_crate::Client`. Publishing the *same* item at two public paths (`the_crate::Client` *and* `the_crate::net::Client`) is what Microsoft Pragmatic Rust Guidelines (M-SINGLE-ITEM-PATH) call a split identity: humans and agents keep both forever. Re-exporting `bytes::Bytes` so callers never depend on `bytes` is the twin foot-gun, unless `Bytes` actually appears in your signatures on purpose (M-FOREIGN-REEXPORTS). Hide the module, re-export the item once, and leak a third-party type only when it is deliberate API currency.

## Bad

```rust
pub mod net {
    pub struct Client;
}

// Two public paths for one type: `net::Client` and `Client`.
pub use net::Client;

fn main() {
    let _ = Client;
    let _ = net::Client;
}
```

## Good

```rust
mod net {
    pub struct Client;
}

pub use net::Client;

fn main() {
    let _ = Client;
}
```

## Foreign Types

Re-export a type from another crate only when that type is already in your public signatures and you are willing to semver-track that crate. Otherwise callers add the dependency themselves (`api-std-types-boundary`).

```rust
// This crate's contract *is* a status code. The wrapper is ours; a real
// `pub use http::StatusCode` follows the same rule when `http` is intentional.
pub struct StatusCode(pub u16);

pub fn not_found() -> StatusCode {
    StatusCode(404)
}

fn main() {
    let _ = not_found();
}
```

## Feature-Gated Re-exports

Name every item. A feature may add re-exports; it must not glob a module into the root.

```rust
mod blocking {
    pub struct BlockingClient;
}

#[cfg(feature = "blocking")]
pub use blocking::BlockingClient;

pub struct Client;

fn main() {
    let _ = Client;
}
```

## See Also

- [proj-no-glob-reexport](proj-no-glob-reexport.md) - never `pub use foo::*` across modules
- [proj-prelude-module](proj-prelude-module.md) - a prelude is the exception, not a second public path
- [doc-inline-reexport](doc-inline-reexport.md) - `#[doc(inline)]` the one path you chose
- [api-std-types-boundary](api-std-types-boundary.md) - most foreign types should not appear at all
- [api-non-exhaustive](api-non-exhaustive.md) - the public surface you flattened still needs a stability story
- [proj-pub-crate-internal](proj-pub-crate-internal.md) - keep the un-exported tree `pub(crate)`
