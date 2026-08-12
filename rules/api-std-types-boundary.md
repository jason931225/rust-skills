# api-std-types-boundary

> Keep third-party types out of the public surface unless that crate is an intentional part of the contract

## Why It Matters

Every type in a public signature is a semver promise. If `load(path: camino::Utf8PathBuf)` leaks `camino`, your users inherit that crate's major bumps. `std` (`Path`, `Vec`, `Duration`, `io::Error`) does not move out from under them. The Microsoft Pragmatic Rust Guidelines allow a leak when the dependency *is* the product (`http::Uri` in an HTTP client, `serde_json::Value` behind a `json` feature). Feature-gate the rest the way `api-serde-optional` already does for serde. This is the type-identity twin of `api-no-wrapper-params`.

## Bad

```rust
pub struct Utf8PathBuf(String);

pub fn load_config(path: Utf8PathBuf) -> String {
    path.0
}
```

## Good

```rust
use std::path::{Path, PathBuf};
use std::time::Duration;

pub fn load_config(path: impl AsRef<Path>) -> PathBuf {
    path.as_ref().to_path_buf()
}

pub fn timeout() -> Duration {
    Duration::from_secs(5)
}

fn main() {
    let _ = load_config("app.toml");
    let _ = timeout();
}
```

## See Also

- [api-serde-optional](api-serde-optional.md) - when a third-party type *is* the contract, put it behind a feature
- [api-no-wrapper-params](api-no-wrapper-params.md) - do not leak ownership wrappers either
- [api-impl-asref](api-impl-asref.md) - accept `AsRef<Path>` / `AsRef<str>` instead of a foreign path type
