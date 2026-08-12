# const-named-magic

> Give production magic numbers a named `const` and a comment that says why that value

## Why It Matters

`60 * 60 * 24` is obviously a day; it is not obvious why this call may wait a day, or what breaks if someone shortens it. The Microsoft Pragmatic Rust Guidelines require a named constant plus a note covering the choice, the side effects of changing it, and any external system that shares the value. Inline literals hide that contract from rustdoc and from every other call site.

## Bad

```rust
use std::time::Duration;

fn wait_timeout(limit: Duration) -> Duration {
    limit
}

fn main() {
    let _ = wait_timeout(Duration::from_secs(60 * 60 * 24));
}
```

## Good

```rust
use std::time::Duration;

/// Upper bound for a single upstream attempt.
///
/// Sized from `api.example.com` idle timeouts. Values below 30s abort
/// in-flight work the peer still considers live.
const UPSTREAM_SERVER_TIMEOUT: Duration = Duration::from_secs(60 * 60 * 24);

fn wait_timeout(limit: Duration) -> Duration {
    limit
}

fn main() {
    let _ = wait_timeout(UPSTREAM_SERVER_TIMEOUT);
}
```

## See Also

- [const-vs-static](const-vs-static.md) - use `const` for an inlined number, `static` only when you need an address
- [name-consts-screaming](name-consts-screaming.md) - `SCREAMING_SNAKE_CASE` marks the value as a policy knob
- [doc-all-public](doc-all-public.md) - the comment belongs on the constant, not only at the use site
