# test-util-feature

> Gate mocks, invariant bypasses, and fake data behind an explicit test-only Cargo feature

## Why It Matters

A `skip_hostname_check` method that ships in the default build is a production foot-gun. Under Microsoft Pragmatic Rust Guidelines (M-TEST-UTIL), every testing affordance — mocks, seedable clocks, inspect-secret helpers — belongs behind one clearly named feature (commonly `test-util` or `testing`) that applications enable only in `[dev-dependencies]` / test crates. Combine it with `#[cfg(feature = "...")]` so the symbols do not exist in release rlibs. `clippy::disallowed_methods` can ban the bypass outside that cfg.

## Bad

```rust
pub struct SmtpClient {
    pub skip_host: bool,
}

impl SmtpClient {
    pub fn new() -> Self {
        Self { skip_host: false }
    }

    pub fn skip_hostname_check(&mut self) {
        self.skip_host = true;
    }
}
```

## Good

```rust
pub struct SmtpClient {
    skip_host: bool,
}

impl SmtpClient {
    pub fn new() -> Self {
        Self { skip_host: false }
    }

    #[cfg(feature = "test-util")]
    pub fn skip_hostname_check(&mut self) {
        self.skip_host = true;
    }

    pub fn verifies_peer(&self) -> bool {
        !self.skip_host
    }
}

fn main() {
    let client = SmtpClient::new();
    assert!(client.verifies_peer());
}
```

## See Also

- [test-mock-traits](test-mock-traits.md) - inject fakes through traits; gate the fake constructors
- [proj-feature-additive](proj-feature-additive.md) - a test feature may add items, never remove production checks by default
- [lint-cfg-check](lint-cfg-check.md) - declare the feature so a typo does not silently drop the gate
