# type-time-sample-once

> Read the clock once per operation and pass the value down

## Why It Matters

Every call to `now()` returns a different answer. Code that calls it in three
places while handling one request can log one timestamp, store another, and
compute an expiry from a third — so a record's "created" and "expires" fields
disagree by microseconds, and near a boundary they disagree by a day. A single
sample taken at the start of the operation makes all derived values consistent
by construction, and it makes the operation testable, because the sample can be
supplied instead of observed.

## Bad

```rust
fn issue_token(user: &User) -> Token {
    Token {
        // Three different instants, and the report may name a different day
        // than the record if the request straddles midnight
        issued_at: Utc::now(),
        expires_at: Utc::now() + Duration::hours(1),
        report_day: Utc::now().date_naive(),
    }
}
```

## Good

```rust
use std::time::{Duration, SystemTime};

#[derive(Debug, PartialEq)]
pub struct Token {
    pub issued_at: SystemTime,
    pub expires_at: SystemTime,
}

/// The clock is an input, so every field derives from one reading and the
/// function can be tested without waiting.
pub fn issue_token(now: SystemTime, lifetime: Duration) -> Token {
    Token { issued_at: now, expires_at: now + lifetime }
}

fn main() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    let token = issue_token(now, Duration::from_secs(3600));

    assert_eq!(token.issued_at, now);
    assert_eq!(
        token.expires_at.duration_since(token.issued_at).expect("later"),
        Duration::from_secs(3600),
        "both fields derive from the same reading"
    );

    // Deterministic: the same input gives the same token, with no clock read.
    assert_eq!(issue_token(now, Duration::from_secs(3600)), token);
}
```

## Key Points

- Sample at the edge — request entry, job start, command start — and pass the
  value through the call chain or the request context.
- A function that takes the time as a parameter needs no clock injection and no
  sleeping in tests.
- Batch work should also share one sample, so every item in a run carries the
  same logical timestamp.
- Measurement is separate: durations come from a monotonic reading, and that
  too is taken once at the start.
- Where a long operation genuinely needs a fresh reading, take it deliberately
  and name why.

## See Also

- [type-time-domain](type-time-domain.md) - which clock the sample comes from
- [test-mock-traits](test-mock-traits.md) - supplying the clock when it cannot be a parameter
- [api-idempotency-key](api-idempotency-key.md) - stored records whose fields must agree
- [async-durable-worker](async-durable-worker.md) - one sample per claimed unit of work
