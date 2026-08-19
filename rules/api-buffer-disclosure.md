# api-buffer-disclosure

> Disclose only the bytes a request actually filled, never the reused buffer's full length

## Why It Matters

A buffer sized once and reused across requests keeps whatever a previous
request wrote in it. A response that copies out `buffer.len()` or
`buffer.capacity()` bytes — instead of the count the current operation
actually filled — sends the tail of that leftover data to whoever asked. This
is the Heartbleed shape: every access is in-bounds and every read is memory
safe, so the bug is invisible to the borrow checker, Miri, and a sanitizer
alike. The bytes at fault were legitimately written by someone else's
request, earlier, into memory this request is now allowed to read from
honestly — the mistake is entirely in how much of it gets disclosed.

## Contract

- Track how many bytes the current operation actually wrote — the count a
  parser consumed, the length a peer's request specified and you validated,
  the return value of a fill call — and disclose exactly that many, never
  `buffer.len()` or `.capacity()` as a proxy for it.
- Do not trust a peer-supplied length claim by itself as the count to copy or
  disclose; validate it against how much data actually exists before using it
  to bound a read or a response.
- When a buffer must be reused across requests for performance, either
  zero the unused tail before it can be read, or slice to the written count
  before it is copied, serialized, or sent — do it at every exit point, not
  only the common one.
- Treat "reused allocation" and "public buffer contents" as separate
  concerns: reuse is about avoiding an allocator call, not about what is safe
  to disclose from what the allocation currently holds.
- Prefer an API shape where the fill operation returns a slice bounded to
  what it wrote (`&buf[..n]`) so the caller cannot reach for `.len()` by
  habit; a `Vec` truncated to the written length is safer than a fixed buffer
  with a separately tracked count that can drift.

## Bad

```rust
/// Reuses one buffer across requests to avoid allocating per request.
struct EchoServer {
    scratch: Vec<u8>,
}

impl EchoServer {
    fn respond(&mut self, request_len: usize) -> &[u8] {
        // `request_len` is whatever the peer claims to have sent, and the
        // response discloses the buffer's full length, not what this
        // request wrote — the tail is still whatever a previous, larger
        // request left behind.
        &self.scratch[..self.scratch.len()]
    }
}
```

## Good

```rust
/// Reuses one buffer across requests, but every response is sliced to
/// exactly the bytes this call wrote — the previous request's leftover
/// tail is never part of what a caller can observe.
struct EchoServer {
    scratch: Vec<u8>,
}

impl EchoServer {
    fn new() -> Self {
        Self { scratch: vec![0u8; 4096] }
    }

    /// `payload` is the actual bytes to echo; its length, not any
    /// caller-claimed length, is what gets copied and disclosed.
    fn respond(&mut self, payload: &[u8]) -> &[u8] {
        let written = payload.len().min(self.scratch.len());
        self.scratch[..written].copy_from_slice(&payload[..written]);
        &self.scratch[..written]
    }
}

fn main() {
    let mut server = EchoServer::new();

    // A first, larger request leaves its bytes in the shared buffer.
    let first = server.respond(b"a much larger first payload here");
    assert_eq!(first, b"a much larger first payload here");

    // A second, shorter request must not disclose the first request's
    // leftover tail — only the bytes this response actually wrote.
    let second = server.respond(b"hi");
    assert_eq!(second, b"hi");
    assert_eq!(second.len(), 2, "the response is bounded to what was written, not the buffer's capacity");
}
```

## Failure Tests

- a shorter request following a longer one discloses only its own bytes, not
  the previous request's leftover tail;
- the disclosed slice's length always equals the bytes actually written, never
  the buffer's capacity or its previous length;
- a peer-supplied length claim larger than the data actually available is
  rejected or clamped, not trusted as the copy count;
- every early-return path from the fill operation still bounds disclosure to
  what was written before that point, not to the buffer's full size.

## See Also

- [type-secret-material](type-secret-material.md) - wiping on drop is a different defense; this rule is about bounding disclosure in the first place
- [err-short-read](err-short-read.md) - trusting a byte count from the wrong source is the same category of mistake in the read direction
- [mem-with-capacity](mem-with-capacity.md) - capacity and length answer different questions; disclosure must use length
- [api-resource-limits](api-resource-limits.md) - bounding an untrusted length claim before it drives any buffer operation
- [obs-no-sensitive-data](obs-no-sensitive-data.md) - the same discipline applied to logs instead of responses
