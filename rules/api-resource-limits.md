# api-resource-limits

> Give every request an explicit ceiling on bytes, time, and concurrency, and reject past it

## Why It Matters

Denial of service is rarely exotic: one unbounded body, one decompression
ratio, or one slow upstream is enough to exhaust memory or occupy every worker
until legitimate traffic fails. Rust removes memory-corruption bugs, not
resource exhaustion — and on an async runtime a handful of requests that block
or allocate without limit can stall an entire executor. Limits belong in the
code path, expressed as numbers the operator can see, not as an assumption
that clients are well behaved.

## Contract

- Set a maximum body size per route and enforce it while reading, not after.
- Cap decompressed output and the compression ratio; a small compressed payload
  can expand without bound.
- Bound collection sizes derived from input: array lengths, page sizes, batch
  counts, and any length prefix that drives an allocation.
- Give every inbound request and every outbound call a deadline.
- Cap in-flight work with a semaphore or a bounded queue so overload sheds load
  instead of queueing without limit.
- Reject over-limit input with a specific, documented status before doing
  domain work, and emit a counter for it — silent truncation is data loss.

## Bad

```rust
async fn upload(mut body: Body) -> Result<Response, Error> {
    // the client decides how much memory this costs
    let bytes = body.read_to_end().await?;
    store(bytes).await
}
```

## Good

```rust
use std::io::{self, Read};

pub const MAX_BODY_BYTES: u64 = 1 << 20; // 1 MiB: largest accepted submission

#[derive(Debug)]
pub enum IntakeError {
    TooLarge,
    Read(io::Error),
}

/// Reads at most `MAX_BODY_BYTES`, and fails rather than truncating when the
/// source has more to give.
pub fn read_bounded<R: Read>(source: R) -> Result<Vec<u8>, IntakeError> {
    let mut limited = source.take(MAX_BODY_BYTES + 1);
    let mut buffer = Vec::new();
    limited.read_to_end(&mut buffer).map_err(IntakeError::Read)?;
    if buffer.len() as u64 > MAX_BODY_BYTES {
        return Err(IntakeError::TooLarge);
    }
    Ok(buffer)
}

fn main() {
    let small = vec![b'x'; 16];
    assert_eq!(read_bounded(&small[..]).map(|b| b.len()).ok(), Some(16));

    let oversized = vec![b'x'; (MAX_BODY_BYTES + 1) as usize];
    assert!(matches!(read_bounded(&oversized[..]), Err(IntakeError::TooLarge)));
}
```

`take` bounds the read itself, so the oversized case never allocates more than
one byte beyond the limit. Reading everything first and checking `len()`
afterwards has already paid the cost the limit exists to prevent.

## Failure Tests

- a body one byte over the limit is rejected, and nothing is stored;
- a chunked or streaming body is cut off at the limit rather than buffered;
- a highly compressible payload cannot exceed the decompressed cap;
- a request that stalls mid-body hits the deadline and releases its worker;
- with the concurrency cap saturated, excess requests are shed with a documented
  status instead of queueing without bound.

## See Also

- [api-extract-or-reject](api-extract-or-reject.md) - validate shape before effects; this bounds cost
- [async-bounded-channel](async-bounded-channel.md) - backpressure instead of unbounded growth
- [async-bounded-dependency](async-bounded-dependency.md) - deadlines and admission for outbound work
- [async-yield-cpu](async-yield-cpu.md) - keep one request from monopolising an executor thread
- [api-outbound-target](api-outbound-target.md) - the same ceilings apply to responses you fetch
