# err-short-read

> Trust the byte count a read returns, not the length you asked for

## Why It Matters

`Read::read` is permitted to return fewer bytes than the buffer holds, and
routinely does: a socket delivers one segment, a pipe delivers what the writer
has flushed so far, a file near its end delivers the remainder. Code that
ignores the returned count and processes the whole buffer emits whatever was
left there before — usually zeros on the first pass, stale data on a reused
buffer — as if it were input. Nothing panics and nothing logs; the corruption
shows up later, in a checksum mismatch or a record that decodes to nonsense.

## Bad

```rust
fn read_header(socket: &mut TcpStream) -> io::Result<Header> {
    let mut buffer = [0u8; 16];
    socket.read(&mut buffer)?;      // may have filled 3 bytes
    Ok(Header::parse(&buffer))      // ...and 13 zeros are parsed as data
}
```

## Good

```rust
use std::io::{self, Read};

/// The whole buffer is required, so a short stream is an error, not a
/// silently zero-padded record.
fn read_exactly(reader: &mut impl Read, buffer: &mut [u8]) -> io::Result<()> {
    reader.read_exact(buffer)
}

/// A short read is expected here, so the count decides what is valid.
fn read_available(reader: &mut impl Read, buffer: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        match reader.read(&mut buffer[filled..])? {
            0 => break, // end of stream
            read => filled += read,
        }
    }
    Ok(filled)
}

fn main() {
    let source = b"abc";

    let mut buffer = [0u8; 8];
    let filled = read_available(&mut &source[..], &mut buffer).expect("read");
    assert_eq!(filled, 3);
    // Slice by what arrived. `&buffer[..8]` would append five zero bytes.
    assert_eq!(&buffer[..filled], b"abc");

    let mut required = [0u8; 8];
    let outcome = read_exactly(&mut &source[..], &mut required);
    assert_eq!(
        outcome.unwrap_err().kind(),
        io::ErrorKind::UnexpectedEof,
        "a truncated stream must fail, not decode as padded data"
    );
}
```

## Handling Partial Reads

- `read_exact` is the right default when the length is part of the format; it
  reports `UnexpectedEof` instead of leaving the tail untouched.
- When a short read is legitimate, loop until the buffer is full or the reader
  returns `Ok(0)`, and treat `Ok(0)` as end of stream, not as an error.
- Slice every downstream operation by the returned count. A buffer reused
  across iterations makes stale bytes look like fresh input.
- `Write::write` has the same contract in reverse — use `write_all` unless the
  partial count is handled explicitly.
- Watch for this at every framing boundary: length prefixes, fixed-size
  headers, and record readers are where the padding becomes data.

## See Also

- [err-result-over-panic](err-result-over-panic.md) - a truncated stream is a recoverable error
- [api-resource-limits](api-resource-limits.md) - bound how much a reader may deliver
- [async-cancel-safety](async-cancel-safety.md) - a cancelled `read` loses the bytes it had consumed
- [test-fuzz-target](test-fuzz-target.md) - truncated input is exactly what a fuzzer produces
