# api-password-reset

> Make password change and recovery single-use, time-bounded, rate-limited security workflows

## Why It Matters

A password reset bypasses the existing password by design. Long-lived,
reusable, guessable, or logged reset tokens become alternate credentials.
Changing a password without revoking sessions leaves stolen sessions active.

## Credential Recovery Requirements

- Authenticated password change verifies the current credential and applies the
  same password policy as enrollment.
- Recovery returns the same public response for known and unknown accounts, in
  status, body, and timing.
- Generate a high-entropy token, store only a verifier derived from it, bind it
  to the account and purpose, and set a short expiry.
- Tokens are single-use: redeeming one and updating the password happen in one
  transaction.
- Revoke other sessions and outstanding reset tokens after success.
- Rate-limit request and redemption by account and network signal, and alert on
  abuse without exposing account existence.
- Deliver recovery links only through a verified channel, and build URLs from
  trusted configuration rather than request headers.
- Never log raw passwords or reset tokens.

## Bad

```rust
fn redeem(record: &ResetRecord, token: &str) -> bool {
    // Plaintext token, no expiry, no single-use check, and `==` leaks the
    // matching prefix through timing
    record.token == token
}
```

## Good

```rust
use std::time::{Duration, SystemTime};

#[derive(Debug, PartialEq)]
pub enum Redeem {
    Ok,
    Expired,
    AlreadyUsed,
    Rejected,
}

pub struct ResetRecord {
    account: u64,
    /// Derived from the token; the token itself is never stored.
    verifier: [u8; 4],
    expires_at: SystemTime,
    used: bool,
}

/// Stand-in for a real KDF over the token.
fn derive(token: &[u8]) -> [u8; 4] {
    let mut out = [0u8; 4];
    for (index, byte) in token.iter().enumerate() {
        out[index % 4] ^= *byte;
    }
    out
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Every check happens before the record is consumed, and consuming it is what
/// makes the token single-use.
pub fn redeem(record: &mut ResetRecord, account: u64, token: &[u8], now: SystemTime) -> Redeem {
    if record.used {
        return Redeem::AlreadyUsed;
    }
    if now > record.expires_at {
        return Redeem::Expired;
    }
    if record.account != account || !constant_time_eq(&record.verifier, &derive(token)) {
        return Redeem::Rejected;
    }
    record.used = true;
    Redeem::Ok
}

fn main() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
    let mut record = ResetRecord {
        account: 7,
        verifier: derive(b"token-abc"),
        expires_at: now + Duration::from_secs(900),
        used: false,
    };

    assert_eq!(redeem(&mut record, 7, b"token-xyz", now), Redeem::Rejected);
    assert_eq!(redeem(&mut record, 7, b"token-abc", now), Redeem::Ok);
    // Single use: the second redemption of a valid token fails.
    assert_eq!(redeem(&mut record, 7, b"token-abc", now), Redeem::AlreadyUsed);
}
```

## Recovery Cases To Test

- unknown and known accounts have the same status/body;
- expired, replayed, modified, and wrong-account tokens fail;
- two concurrent redemptions produce one success;
- successful reset revokes old sessions and remaining tokens;
- email/link construction cannot be changed by a forged Host header.

## See Also

- [api-password-auth](api-password-auth.md) - hash and verify the new credential
- [api-session-security](api-session-security.md) - revoke active sessions after reset
- [conc-db-transaction-boundary](conc-db-transaction-boundary.md) - consume token and update credential atomically
- [api-tls-required](api-tls-required.md) - reset links and credential submission require TLS
- [obs-no-sensitive-data](obs-no-sensitive-data.md) - redact reset tokens and account data
