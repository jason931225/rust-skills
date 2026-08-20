# type-capability-token

> Make a privileged operation take an unforgeable token, so authority appears in the signature

## Why It Matters

Checking a privilege flag inside each dangerous function puts the burden on
whoever writes the next one: the check is easy to forget, and forgetting it is
invisible at the call site. A capability token inverts that — the operation
takes a value only the authenticating path can mint, so a caller without
authority cannot even name the argument. Authority becomes part of the type
signature rather than a convention, and a reviewer can see which functions
require it by reading their parameters.

## Bad

```rust
impl Device {
    fn erase_firmware(&mut self, session: &Session) -> Result<(), Error> {
        // Every privileged method repeats this, and the one that forgets it
        // compiles exactly as well as the ones that do not
        if !session.is_admin {
            return Err(Error::Forbidden);
        }
        self.erase()
    }
}
```

## Good

```rust
/// Proof of administrative authority. The private field means only this
/// module's authenticating constructor can produce one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AdminCap {
    _private: (),
}

#[derive(Debug, PartialEq)]
pub enum AuthError {
    BadCredentials,
}

impl AdminCap {
    /// The single mint. Every other path to an `AdminCap` is a compile error.
    pub fn authenticate(token: &str) -> Result<Self, AuthError> {
        if token == "operator-secret" {
            Ok(Self { _private: () })
        } else {
            Err(AuthError::BadCredentials)
        }
    }
}

pub struct Device {
    erased: bool,
}

impl Device {
    /// The capability is a parameter, so authority is visible in the signature
    /// and cannot be forgotten inside the body.
    pub fn erase_firmware(&mut self, _proof: AdminCap) {
        self.erased = true;
    }
}

fn main() {
    let mut device = Device { erased: false };

    assert_eq!(AdminCap::authenticate("guess"), Err(AuthError::BadCredentials));

    let proof = AdminCap::authenticate("operator-secret").expect("authenticated");
    device.erase_firmware(proof);
    assert!(device.erased);

    // `Device::erase_firmware(&mut device, AdminCap { _private: () })` does not
    // compile outside this module: the field is private, so the token cannot
    // be forged.
}
```

## Token Design Constraints

- The unforgeability comes from a private field, not from the type being
  zero-sized; a token with all-public fields is a suggestion.
- A capability is reusable within its scope — that is what distinguishes it
  from a single-use token, which is consumed by value.
- Express tiers with separate types or a supertrait chain rather than a level
  field, so a lower tier cannot stand in for a higher one.
- Bind the token's lifetime to the session it came from where revocation
  matters; a `'static` capability outlives the authority that granted it.
- This complements runtime authorization, it does not replace it: the type
  proves the caller held authority, not that the policy still permits the
  action.

## See Also

- [api-authz-fail-closed](api-authz-fail-closed.md) - the runtime decision this makes visible
- [type-single-use-token](type-single-use-token.md) - the at-most-once sibling, consumed by value
- [api-typestate](api-typestate.md) - encoding a protocol in types rather than flags
- [api-sealed-trait](api-sealed-trait.md) - the other way to restrict who may construct
