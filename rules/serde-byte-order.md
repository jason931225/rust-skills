# serde-byte-order

> Declare a byte order for every multi-byte value that leaves the process, and convert explicitly at the boundary

## Why It Matters

`to_ne_bytes`, a `#[repr(C)]` struct written straight to disk, or a length
prefix built from host memory all encode whatever order the current CPU
happens to use. The result reads back correctly on the machine that wrote it
and silently wrong on any other — a length becomes enormous, a timestamp jumps
centuries, a checksum never matches. Because the same binary usually reads
what it wrote during development, this survives every test that does not cross
architectures.

## Bad

```rust
fn write_len(file: &mut File, len: u32) -> io::Result<()> {
    // Host order: correct on x86_64, wrong when a big-endian reader picks it up
    file.write_all(&len.to_ne_bytes())
}
```

## Good

```rust
use std::io::{self, Read, Write};

/// The wire format is big-endian (network order); the choice is part of the
/// format, not of the machine that happens to run the encoder.
fn write_len(sink: &mut impl Write, len: u32) -> io::Result<()> {
    sink.write_all(&len.to_be_bytes())
}

fn read_len(source: &mut impl Read) -> io::Result<u32> {
    let mut bytes = [0u8; 4];
    source.read_exact(&mut bytes)?;
    Ok(u32::from_be_bytes(bytes))
}

fn main() {
    let mut encoded = Vec::new();
    write_len(&mut encoded, 0x0102_0304).expect("write");

    // The encoding is fixed by the format, so the bytes are predictable and
    // testable on any host.
    assert_eq!(encoded, [0x01, 0x02, 0x03, 0x04]);
    assert_eq!(read_len(&mut &encoded[..]).expect("read"), 0x0102_0304);
}
```

## Key Points

- Pick one order per format and document it. Network protocols conventionally
  use big-endian; many on-disk formats choose little-endian because most
  hardware is.
- Convert with `to_be_bytes`/`from_le_bytes` and their siblings at the
  boundary. Never `to_ne_bytes` in anything persisted or transmitted.
- Floats need the same treatment — convert through `to_bits` and encode the
  integer.
- Do not write struct memory directly: padding and field order are not part of
  a stable contract even with `#[repr(C)]`.
- Test with fixed byte arrays, as above. A round-trip through the same host
  passes under either order and proves nothing.

## See Also

- [serde-format-version](serde-format-version.md) - the other half of a durable binary format
- [num-cast-try-from](num-cast-try-from.md) - narrowing a decoded length is a fallible conversion
- [err-short-read](err-short-read.md) - decode only bytes that actually arrived
- [mem-assert-type-size](mem-assert-type-size.md) - why in-memory layout is not a wire format
