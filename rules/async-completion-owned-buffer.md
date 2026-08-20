# async-completion-owned-buffer

> Completion-based I/O must take the buffer by value and hand it back with the result; do not wrap it in the readiness `AsyncRead`/`AsyncWrite` traits

## Why It Matters

The `AsyncRead`/`AsyncWrite` traits are readiness interfaces: `poll_read`
lends the implementation a `&mut [u8]` for the duration of that one call, and
the borrow ends when `poll_read` returns. Completion-based engines — io_uring,
Windows overlapped IOCP, RDMA — work the other way round: the kernel or NIC
takes ownership of the buffer at submission and writes into it at some later
point, signalled by a completion event. Implementing a readiness trait over a
completion engine therefore requires retaining a pointer to the caller's slice
after `poll_read` has returned `Pending`, which is exactly the aliasing the
borrow ended. The fix is not a smarter lifetime, because there is no lifetime
that covers "until the kernel says so" — the interface itself has to change to
pass the buffer by value and return it alongside the result.

## Ownership At The Submission Boundary

- Take the buffer by value at submission and return it to the caller with the
  operation's result; a completion API's read is `submit(buf) -> (buf, result)`,
  not `poll_read(&mut buf)`.
- Do not implement `AsyncRead`/`AsyncWrite` (or another readiness trait) over
  a completion engine by stashing the borrowed slice's pointer past the
  `Pending` return. The borrow is over; retaining it is undefined behavior
  regardless of whether the kernel has written yet.
- Where an adapter to the readiness traits is genuinely required — an existing
  ecosystem expects `AsyncRead` — give the adapter its own internal buffer
  that it owns for the operation's whole lifetime, and copy into the caller's
  slice only once the completion has been observed. That copy is the cost of
  the impedance mismatch; make it explicit rather than eliding it unsoundly.
- Treat cancellation as a distinct problem from readiness I/O: dropping the
  future does not retract a submitted operation, so the buffer must stay alive
  and un-aliased until the completion arrives even if nobody is waiting.
- Keep the buffer out of reach for the whole in-flight window, not merely for
  the duration of a poll — the guard shape in
  [type-exclusive-occupancy-guard](type-exclusive-occupancy-guard.md) is how
  the ownership half is enforced; this rule is about which interface shape
  makes that possible at all.

## Bad

```rust
use std::task::{Context, Poll};

struct Submission {
    // Retained from a caller's `&mut [u8]` after `poll_read` returned. The
    // borrow that produced it has ended, so the kernel writing through this
    // pointer later aliases memory the caller may have reused or dropped.
    kernel_target: *mut u8,
    len: usize,
}

struct CompletionReader {
    pending: Option<Submission>,
}

impl CompletionReader {
    // The shape of the mistake: a readiness signature over a completion engine.
    fn poll_read(&mut self, _cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<usize> {
        if self.pending.is_none() {
            self.pending = Some(Submission { kernel_target: buf.as_mut_ptr(), len: buf.len() });
            return Poll::Pending; // the &mut [u8] borrow ends here
        }
        Poll::Ready(0)
    }
}
```

## Good

```rust
use std::io;

/// A submitted operation. The engine owns `buffer` until completion, so
/// nothing else can read or write it in the meantime.
pub struct InFlight {
    buffer: Vec<u8>,
    requested: usize,
}

pub struct CompletionEngine;

impl CompletionEngine {
    /// Takes the buffer by value: from here until `complete`, the caller has
    /// no way to reach these bytes, which is what makes the kernel's later
    /// write sound.
    pub fn submit_read(&self, buffer: Vec<u8>, requested: usize) -> InFlight {
        InFlight { requested: requested.min(buffer.len()), buffer }
    }

    /// Hands the buffer back together with the result, so a caller that wants
    /// to reuse the allocation gets it only after the operation is finished.
    pub fn complete(&self, mut in_flight: InFlight) -> (Vec<u8>, io::Result<usize>) {
        // Stands in for observing the completion event and the bytes the
        // engine wrote into the buffer it owned.
        for slot in in_flight.buffer[..in_flight.requested].iter_mut() {
            *slot = 0xab;
        }
        let written = in_flight.requested;
        (in_flight.buffer, Ok(written))
    }
}

fn main() {
    let engine = CompletionEngine;
    let buffer = vec![0u8; 8];

    let in_flight = engine.submit_read(buffer, 4);
    // `buffer` has moved: there is no way to observe or mutate the bytes
    // while the engine owns them. Any attempt does not compile.

    let (buffer, result) = engine.complete(in_flight);
    assert_eq!(result.expect("the read completes"), 4);
    assert_eq!(&buffer[..4], &[0xab; 4]);
    assert_eq!(&buffer[4..], &[0u8; 4], "the engine wrote only what was requested");
}
```

## Cases To Pin In Tests

- the buffer is unreachable between submission and completion — confirmed by
  the move, not by a runtime check;
- the returned buffer carries the bytes the engine wrote, and the untouched
  tail is unchanged;
- an operation whose future is dropped before completion still keeps the
  buffer alive until the completion is observed, rather than freeing it;
- an adapter exposing `AsyncRead` over the engine copies from its own
  internal buffer and never retains the caller's slice past a `Pending`.

## See Also

- [type-exclusive-occupancy-guard](type-exclusive-occupancy-guard.md) - the ownership guard that keeps the in-flight buffer un-aliased
- [async-poll-contract](async-poll-contract.md) - the readiness contract this rule explains a completion engine cannot honestly satisfy
- [api-impl-io](api-impl-io.md) - the readiness traits themselves, and when accepting them is right
- [async-cancel-safety](async-cancel-safety.md) - dropping the future does not retract a submitted operation
- [unsafe-pointer-provenance](unsafe-pointer-provenance.md) - the raw pointer handed to the kernel carries the provenance of the allocation it was derived from
