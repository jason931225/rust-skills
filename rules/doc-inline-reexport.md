# doc-inline-reexport

> Put `#[doc(inline)]` on `pub use` of items you own so rustdoc shows them next to their siblings

## Why It Matters

A bare `pub use crate::net::Client` renders as a re-export block. Readers hunting for `Client` in the crate root miss it, and the type's docs live two clicks away. Following Microsoft Pragmatic Rust Guidelines (M-DOC-INLINE), first-party re-exports get `#[doc(inline)]` so the item appears as if it were defined there. Leave third-party types *without* inline so it stays obvious they come from `bytes` or `http`. This pairs with one-canonical-path (`proj-pub-use-reexport`): inline the one public path, do not publish two.

## Bad

```rust
mod net {
    pub struct Client;
}

pub use net::Client;
```

## Good

```rust
mod net {
    pub struct Client;
}

#[doc(inline)]
pub use net::Client;

fn main() {
    let _ = Client;
}
```

## See Also

- [proj-pub-use-reexport](proj-pub-use-reexport.md) - one public path; inline that path
- [proj-no-glob-reexport](proj-no-glob-reexport.md) - `#[doc(inline)]` does not make a glob safe
- [doc-all-public](doc-all-public.md) - inlined items still need their own rustdoc
