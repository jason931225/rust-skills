# proj-prelude-module

> Scope a `prelude` to large trait-heavy libraries; typical crates should not define one

## Why It Matters

A `prelude` glob (`use foo::prelude::*`) looks cheap until two crates both export `Client` and the build becomes `error[E0659]: Client is ambiguous`. Today's rust-analyzer already inserts named imports. The Microsoft Pragmatic Rust Guidelines therefore tell ordinary libraries not to ship a prelude: it papers over a muddy root and fights every other glob in the crate. rust-skills previously recommended a prelude for common imports; that disagreed with that guidance. The remaining legitimate case is a *large, trait-heavy* library in the style of `std` or `rayon`, where calling the crate at all means bringing many traits into scope. Applications and typical libraries export named items at the root and stop.

## Bad

```rust
// Typical client crate: a prelude whose only job is to hide four root types.
pub struct Client;
pub struct Config;
pub struct Error;

pub mod prelude {
    pub use crate::{Client, Config, Error};
}

fn main() {
    let _ = prelude::Client;
}
```

## Good

```rust
// Typical crate: named exports, no prelude module.
pub struct Client;
pub struct Config;
pub struct Error;

pub fn connect(_cfg: &Config) -> Result<Client, Error> {
    Ok(Client)
}

fn main() {
    let _ = connect(&Config);
}
```

## When a Prelude Is Justified

A crate that is *about* a family of traits (parallel iterators, parser combinators, a web extractor stack) may ship one curated prelude. Keep it small, list every item in the module docs, and treat removals as breaking. Do not glob the rest of the crate into it (`proj-no-glob-reexport`).

```rust
pub trait ParallelIterator {
    fn for_each<F: Fn(Self::Item)>(self, f: F)
    where
        Self: Sized;
    type Item;
}

impl<T> ParallelIterator for Vec<T> {
    type Item = T;

    fn for_each<F: Fn(T)>(self, f: F) {
        for item in self {
            f(item);
        }
    }
}

/// Traits that must be in scope to use this crate's iterators.
pub mod prelude {
    pub use crate::ParallelIterator;
}

fn main() {
    use prelude::ParallelIterator;
    vec![1, 2, 3].for_each(|_| {});
}
```

## See Also

- [proj-pub-use-reexport](proj-pub-use-reexport.md) - one named public path, not a glob
- [proj-no-glob-reexport](proj-no-glob-reexport.md) - a prelude is still a list, never `pub use crate::*`
- [proj-mod-by-feature](proj-mod-by-feature.md) - fix the module layout before inventing a prelude
- [api-extension-trait](api-extension-trait.md) - extension traits are the usual reason a large crate needs a prelude
