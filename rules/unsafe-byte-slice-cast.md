# unsafe-byte-slice-cast

> Reinterpret bytes as a typed value only through a length- and alignment-checked conversion

## Why It Matters

Casting `&[u8]` to `&T` with a pointer cast is undefined behaviour unless the
slice is long enough, correctly aligned for `T`, and every bit pattern in it is
valid for `T`. Each of those fails quietly: a short slice reads past the end, a
misaligned read is UB even on hardware that tolerates it, and an invalid
discriminant or `bool` is UB the moment it exists. Network buffers and
memory-mapped files are exactly where this is attempted and exactly where the
alignment guarantee is absent.

## Bad

```rust
fn header(buffer: &[u8]) -> &Header {
    // No length check, no alignment check, and Header's fields may have
    // validity invariants the bytes do not satisfy
    unsafe { &*(buffer.as_ptr() as *const Header) }
}
```

## Good

```rust
#[derive(Debug, PartialEq)]
pub struct Header {
    pub version: u16,
    pub length: u16,
}

#[derive(Debug, PartialEq)]
pub enum DecodeError {
    Truncated,
    UnsupportedVersion(u16),
}

/// Decode field by field from the bytes actually present. No pointer cast, so
/// there is no alignment or validity obligation to discharge.
pub fn decode_header(buffer: &[u8]) -> Result<Header, DecodeError> {
    let bytes: [u8; 4] = buffer.get(..4).ok_or(DecodeError::Truncated)?
        .try_into()
        .map_err(|_| DecodeError::Truncated)?;
    let version = u16::from_be_bytes([bytes[0], bytes[1]]);
    if version != 1 {
        return Err(DecodeError::UnsupportedVersion(version));
    }
    Ok(Header { version, length: u16::from_be_bytes([bytes[2], bytes[3]]) })
}

fn main() {
    let frame = [0x00, 0x01, 0x00, 0x20, 0xff];
    assert_eq!(decode_header(&frame), Ok(Header { version: 1, length: 32 }));

    // A short buffer is an error, not a read past the end.
    assert_eq!(decode_header(&frame[..3]), Err(DecodeError::Truncated));
    // An unknown version is rejected before anything else is trusted.
    assert_eq!(decode_header(&[0x00, 0x09, 0, 0]), Err(DecodeError::UnsupportedVersion(9)));

    // Alignment is why the pointer cast is unsound: a byte slice carries no
    // guarantee that its start is aligned for the target type.
    let unaligned = &frame[1..];
    assert_ne!(unaligned.as_ptr().align_offset(align_of::<u32>()), 0);
}
```

## Alignment And Validity Constraints

- Decoding field by field is almost always fast enough; the compiler folds it
  into the same loads when the bytes are contiguous.
- If a zero-copy view is genuinely required, use a reviewed crate that checks
  alignment and validity in its safe API rather than writing the cast.
- `align_of::<T>()` and `align_offset` are how alignment is checked; a length
  check alone is not sufficient.
- Types with validity invariants — `bool`, `char`, enums, `NonZero*`,
  references — can never be produced from arbitrary bytes, checked or not.
- Byte order is a separate obligation: a checked cast still reads host order.

## See Also

- [serde-byte-order](serde-byte-order.md) - the encoding the decoded fields must agree on
- [unsafe-sound-abstractions](unsafe-sound-abstractions.md) - why the unchecked cast cannot sit behind a safe function
- [err-short-read](err-short-read.md) - decode only the bytes that arrived
- [mem-assert-type-size](mem-assert-type-size.md) - why in-memory layout is not a wire format
