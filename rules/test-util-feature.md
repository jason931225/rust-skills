# test-util-feature

> Gate mocks, invariant bypasses, and fake data behind an explicit test-only Cargo feature

## Why It Matters

A `bypass_certificate_checks` method that ships in the default build is a production foot-gun. The Microsoft Pragmatic Rust Guidelines put every testing affordance — mocks, seedable clocks, inspect-secret helpers — behind one clearly named feature (commonly `test-util` or `testing`) that applications enable only in `[dev-dependencies]` / test crates. Combine it with `#[cfg(feature = "...")]` so the symbols do not exist in release rlibs. `clippy::disallowed_methods` can ban the bypass outside that cfg.

## Bad

```rust
pub struct TlsClient {
    pub skip_verify: bool,
}

impl TlsClient {
    pub fn new() -> Self {
        Self { skip_verify: false }
    }

    pub fn bypass_certificate_checks(&mut self) {
        self.skip_verify = true;
    }
}
```

## Good

```rust
pub struct TlsClient {
    skip_verify: bool,
}

impl TlsClient {
    pub fn new() -> Self {
        Self { skip_verify: false }
    }

    #[cfg(feature = "test-util")]
    pub fn bypass_certificate_checks(&mut self) {
        self.skip_verify = true;
    }

    pub fn verifies_peer(&self) -> bool {
        !self.skip_verify
    }
}

fn main() {
    let client = TlsClient::new();
    assert!(client.verifies_peer());
}
```

## See Also

- [test-mock-traits](test-mock-traits.md) - inject fakes through traits; gate the fake constructors
- [proj-feature-additive](proj-feature-additive.md) - a test feature may add items, never remove production checks by default
- [lint-cfg-check](lint-cfg-check.md) - declare the feature so a typo does not silently drop the gate
