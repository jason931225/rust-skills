# type-secret-material

> Carry credentials in a wrapper type that redacts its `Debug`, withholds `Display`, and wipes on drop

## Why It Matters

Secrets leak by accident, not by design. Structured-logging attributes that
capture function arguments, a derived `Debug` on a config struct, an error
that formats the connection string — each is one line of ordinary code, and
the result is credentials in a log aggregator that many people can read. An
opt-out convention ("remember to skip that field") fails the first time
someone adds a parameter. A type makes the compiler carry the rule: the secret
has no `Display`, its `Debug` is redacted, and reaching the value requires
naming that intent at the call site.

## Contract

- Wrap passwords, API keys, tokens, private keys, and connection strings that
  embed them in a dedicated secret type.
- Implement `Debug` manually to print a placeholder; do not derive it.
- Do not implement `Display`. Exposure happens through one explicit accessor.
- Anything derived from a secret is also a secret — wrap the connection string,
  not just the password.
- Zero the buffer on drop where the platform allows it, and keep the plaintext
  scoped as narrowly as possible.
- Deserialization may fill a secret directly; serialization of one should be
  deliberate and rare.
- Do not put secrets in URLs, query parameters, or span fields, and never
  include them in an error's `Display` output.

## Bad

```rust
#[derive(Debug, serde::Deserialize)]
pub struct DatabaseSettings {
    pub username: String,
    // #[tracing::instrument] or a `{:?}` on this struct prints the password
    pub password: String,
}
```

## Good

```rust
use std::fmt;

/// Types that can overwrite their own buffer before being dropped.
pub trait Wipe {
    fn wipe(&mut self);
}

impl Wipe for String {
    fn wipe(&mut self) {
        // `into_bytes` keeps the same allocation, so zeroing it wipes the text.
        let mut bytes = std::mem::take(self).into_bytes();
        bytes.iter_mut().for_each(|byte| *byte = 0);
    }
}

impl Wipe for Vec<u8> {
    fn wipe(&mut self) {
        self.iter_mut().for_each(|byte| *byte = 0);
    }
}

/// A value that must not appear in logs, traces, or error messages.
pub struct Secret<T: Wipe>(T);

impl<T: Wipe> Secret<T> {
    pub fn new(inner: T) -> Self {
        Self(inner)
    }

    /// The single, greppable point where a secret becomes readable.
    pub fn expose(&self) -> &T {
        &self.0
    }
}

impl<T: Wipe> fmt::Debug for Secret<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Secret([redacted])")
    }
}

impl<T: Wipe> Drop for Secret<T> {
    fn drop(&mut self) {
        // Best-effort wipe; a hardened crate also defeats compiler elision.
        self.0.wipe();
    }
}

#[derive(Debug)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: Secret<String>,
}

fn main() {
    let settings = DatabaseSettings {
        username: "app".to_owned(),
        password: Secret::new("hunter2".to_owned()),
    };
    let rendered = format!("{settings:?}");
    assert!(rendered.contains("Secret([redacted])"));
    assert!(!rendered.contains("hunter2"));
    assert_eq!(settings.password.expose(), "hunter2");
}
```

In production use a maintained crate (`secrecy` and `zeroize` are the common
pair) rather than a local copy; the value here is the shape — no `Display`,
redacted `Debug`, one named exposure point.

## Failure Tests

- `{:?}` on the enclosing config, request, or error does not contain the secret;
- a structured-logging macro that captures all arguments emits the placeholder;
- serializing the surrounding type does not emit the secret unless explicitly
  requested;
- the secret does not appear in a panic message or an error chain.

## See Also

- [obs-no-sensitive-data](obs-no-sensitive-data.md) - the logging rule this type enforces mechanically
- [type-display-vs-debug](type-display-vs-debug.md) - why withholding `Display` is the point
- [proj-typed-config](proj-typed-config.md) - secrets enter through typed configuration
- [api-crypto-primitives](api-crypto-primitives.md) - key material uses this wrapper
- [api-password-auth](api-password-auth.md) - stored password hashes have their own contract
