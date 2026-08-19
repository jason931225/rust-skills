# err-debug-assert-scope

> Guard internal invariants with `debug_assert!`; validate boundary data with checks that survive `--release`

## Why It Matters

`debug_assert!` compiles to nothing in a release build. Using it on data that
arrives from a file, a socket, a database, or a user means the shipped binary
performs no check at all — the one build where malformed input actually
arrives is the build with the validation removed. The failure is not a panic
in production; it is the absence of a panic, followed by an out-of-bounds
index, a wrong length, or a record that decodes into the wrong type.

## Bad

```rust
fn parse_record(bytes: &[u8]) -> Record {
    // In release this check does not exist: a crafted length walks off the end
    debug_assert!(bytes.len() >= HEADER_LEN);
    Record::from(&bytes[..HEADER_LEN])
}
```

## Good

```rust
const HEADER_LEN: usize = 4;

#[derive(Debug, PartialEq)]
pub enum RecordError {
    Truncated,
}

/// Boundary data: checked in every profile, and the failure is a value.
pub fn parse_record(bytes: &[u8]) -> Result<(&[u8], &[u8]), RecordError> {
    if bytes.len() < HEADER_LEN {
        return Err(RecordError::Truncated);
    }
    let (header, body) = bytes.split_at(HEADER_LEN);

    // Internal invariant: `split_at` cannot return a short prefix. Stating it
    // is free in release and catches a future refactor in tests.
    debug_assert_eq!(header.len(), HEADER_LEN);
    Ok((header, body))
}

fn main() {
    assert_eq!(parse_record(b"abcdBODY"), Ok((&b"abcd"[..], &b"BODY"[..])));
    assert_eq!(parse_record(b"ab"), Err(RecordError::Truncated));
}
```

## Key Points

- The test is provenance, not cost: if the value crossed a trust or storage
  boundary, the check ships.
- `debug_assert!` belongs on invariants your own code establishes a few lines
  earlier — the ones a refactor could break and a test would catch.
- An invariant that `unsafe` code relies on for soundness is never
  debug-only; the release build is where the undefined behaviour happens.
- Prefer returning an error to asserting at all on boundary data; a panic in a
  request handler is a denial-of-service surface.
- Keep `debug_assert!` cheap. An expensive one silently changes the
  performance profile of test and debug builds only.

## See Also

- [err-result-over-panic](err-result-over-panic.md) - boundary failures are values, not panics
- [api-parse-dont-validate](api-parse-dont-validate.md) - turn checked bytes into a type once
- [unsafe-safety-comment](unsafe-safety-comment.md) - soundness preconditions cannot be debug-only
- [perf-release-profile](perf-release-profile.md) - keep the profile you test and the profile you ship aligned
