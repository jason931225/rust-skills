# api-crypto-primitives

> Use vetted authenticated primitives and compare secrets in constant time; never implement your own

## Why It Matters

Cryptographic code that passes its test vectors can still be broken: the
failures that matter are side channels, key handling, and nonce reuse, none of
which a correctness test detects. Unauthenticated block-cipher and MAC
compositions leave room for forgery and oracle attacks, and a byte-by-byte
comparison of a MAC or token leaks the answer through timing. Memory safety
does not make an implementation constant time — and an empirical study of
cryptographic libraries found the majority of their reported vulnerabilities
were memory-safety issues rather than cryptographic ones, which is an argument
for writing crypto in Rust, not for writing your own.

## Contract

- Use a maintained, reviewed implementation. Do not implement primitives.
- Encrypt with an AEAD — AES-256-GCM, ChaCha20-Poly1305, or XChaCha20-Poly1305 —
  so ciphertext is authenticated. Do not compose a cipher and a MAC by hand.
- Never reuse a `(key, nonce)` pair. Use random nonces from a CSPRNG with an
  extended-nonce construction, or a counter you can prove is unique.
- Derive keys with a KDF; do not use a password or a raw shared secret directly.
- Compare secrets, MACs, and tokens in constant time.
- Keep key material in a type that redacts its `Debug` output and wipes on drop;
  scope it to the smallest lifetime that works.
- Treat algorithm agility as versioned data: store the parameters alongside the
  ciphertext so a scheme can be migrated later.

## Bad

```rust
fn verify(expected: &[u8], provided: &[u8]) -> bool {
    // returns as soon as bytes differ: the attacker learns the prefix length
    expected == provided
}
```

## Good

```rust
/// Compares two secrets without an early exit, so the time taken does not
/// depend on where they first differ.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut difference = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        difference |= x ^ y;
    }
    difference == 0
}

fn main() {
    let tag = [0xa1, 0xb2, 0xc3, 0xd4];
    assert!(constant_time_eq(&tag, &[0xa1, 0xb2, 0xc3, 0xd4]));
    assert!(!constant_time_eq(&tag, &[0xa1, 0xb2, 0xc3, 0xd5]));
    assert!(!constant_time_eq(&tag, &[0xa1, 0xb2, 0xc3]));
}
```

The loop above shows the property; in production use a reviewed constant-time
crate (for example `subtle`), because a compiler is free to reintroduce a
branch that the source does not contain. Length is deliberately allowed to
leak — MAC and token lengths are public.

## Failure Tests

- verification rejects a tag that differs only in its final byte;
- encrypting the same plaintext twice produces different ciphertexts;
- a tampered ciphertext, nonce, or associated-data value fails to decrypt;
- decryption of an unauthenticated or truncated message returns an error rather
  than partial plaintext;
- key material does not appear in logs, `Debug` output, or error messages.

## See Also

- [api-password-auth](api-password-auth.md) - password hashing is a separate, memory-hard scheme
- [type-secret-material](type-secret-material.md) - the type that carries keys and tokens
- [api-tls-required](api-tls-required.md) - transport security uses the same rule: no hand-rolled stacks
- [api-session-security](api-session-security.md) - session tokens are secrets compared the same way
- [obs-no-sensitive-data](obs-no-sensitive-data.md) - keep key material out of telemetry
