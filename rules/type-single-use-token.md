# type-single-use-token

> Give an at-most-once permission a type that is neither `Clone` nor `Copy`, so a second use will not compile

## Why It Matters

A nonce, an ephemeral key, a one-time password, or a signed capability is valid
for exactly one use, and the consequences of a second use are severe: nonce
reuse breaks an AEAD outright, a replayed capability performs an action twice.
Enforcing that at runtime — a `used: bool`, a database column — catches it
after the fact and only where the check was remembered. A type that is moved
and never duplicated moves the check to compile time: passing the value to the
operation consumes it, and the second call is a use-after-move error.

## Bad

```rust
#[derive(Clone, Copy)]        // one derive undoes the whole guarantee
struct Nonce([u8; 12]);

fn seal(key: &Key, nonce: Nonce, plaintext: &[u8]) -> Vec<u8> { /* ... */ }

let nonce = Nonce(generate());
let first = seal(&key, nonce, b"a");
let second = seal(&key, nonce, b"b");  // compiles; the AEAD is now broken
```

## Good

```rust
/// Deliberately not `Clone` or `Copy`: the value is a permission, and
/// duplicating it would duplicate the permission.
pub struct Nonce([u8; 12]);

impl Nonce {
    pub fn generate(bytes: [u8; 12]) -> Self {
        Self(bytes)
    }
}

pub struct Sealed {
    pub nonce: [u8; 12],
    pub ciphertext: Vec<u8>,
}

/// Takes the nonce by value, so the caller cannot hold it afterwards.
pub fn seal(nonce: Nonce, plaintext: &[u8]) -> Sealed {
    Sealed { nonce: nonce.0, ciphertext: plaintext.to_vec() }
}

fn main() {
    let nonce = Nonce::generate([7; 12]);
    let sealed = seal(nonce, b"message");
    assert_eq!(sealed.nonce, [7; 12]);

    // seal(nonce, b"again") would not compile: `nonce` was moved above.
    // That is the guarantee — it is enforced by the compiler, not by a flag.
}
```

## Preserving The Single-Use Guarantee

- The guarantee is the *absence* of `Clone` and `Copy`. Adding either later
  deletes it silently, which is why the type deserves a comment saying so and a
  compile-fail test pinning it.
- Take the token by value in the operation that spends it. A `&Nonce` parameter
  leaves the caller holding a reusable value.
- Where the token must cross a serialization boundary, spend it on the way in:
  parse into the move-only type once, at the edge.
- A `Drop` impl can detect an unspent token — a warning that a permission was
  generated and thrown away.
- Runtime single-use enforcement is still required where the token crosses
  processes; the type protects one process from itself.
- A move-only token proves *at most once*, never *at least once*. Dropping
  it unspent still compiles — `#[must_use]` produces a warning, not an
  error — so a required step cannot be enforced this way. When a step must
  actually happen, use typestate so the follow-on operation exists only on
  the type the consuming step returns ([api-typestate](api-typestate.md)),
  or check at runtime; do not read "not `Clone`, taken by value" as
  exactly-once.

## See Also

- [api-crypto-primitives](api-crypto-primitives.md) - why nonce reuse is fatal
- [api-typestate](api-typestate.md) - encoding a state machine in the same way
- [own-copy-small](own-copy-small.md) - when implicit duplication is appropriate, and when it is not
- [api-idempotency-key](api-idempotency-key.md) - the cross-process form of the same problem
