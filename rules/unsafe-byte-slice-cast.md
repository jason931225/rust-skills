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
    // guarantee that its start is aligned for the target type. Force a known
    // base so the demonstration is deterministic rather than depending on
    // where the allocator happened to put a plain array.
    #[repr(align(4))]
    struct Aligned([u8; 8]);
    let backing = Aligned([0x00, 0x01, 0x00, 0x20, 0xff, 0, 0, 0]);
    let unaligned = &backing.0[1..];
    assert_ne!(
        unaligned.as_ptr().align_offset(align_of::<u32>()),
        0,
        "one byte past a 4-aligned base is never 4-aligned"
    );
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

## Packed Fields Are The Same Obligation From The Other Side

A `#[repr(C, packed)]` struct drops padding, so a field can sit at an offset
that does not satisfy its own alignment. Rust does not let you form a reference
to one — `&frame.id` is `error[E0793]: reference to field of packed struct is
unaligned`, and the note says why: creating a misaligned reference is undefined
behaviour *even if that reference is never dereferenced*.

That the compiler rejects it is the important part. Safe code cannot express
this mistake, so the obligation only survives where you have opted out of the
check with a raw pointer:

```rust
#[repr(C, packed)]
struct Frame {
    kind: u8,
    id: u32,
    flags: u16,
}

fn read_id(frame: &Frame) -> u32 {
    // Copying out by value is the ordinary answer; the compiler emits the
    // unaligned load for you and no reference is ever formed.
    frame.id
}

fn read_id_via_pointer(frame: &Frame) -> u32 {
    // `&raw const` produces a pointer without going through a reference, so
    // the misalignment is legal here. The read must still be the unaligned
    // one: `*p` would be UB even though taking `p` was not.
    let p = &raw const frame.id;
    // SAFETY: `p` points at an initialised `u32` inside `frame`, which the
    // borrow guarantees is live; `read_unaligned` imposes no alignment
    // requirement, which is the only reason this is sound for a packed field.
    unsafe { p.read_unaligned() }
}

fn main() {
    let frame = Frame { kind: 7, id: 0x0102_0304, flags: 9 };
    assert_eq!(read_id(&frame), 0x0102_0304);
    assert_eq!(read_id_via_pointer(&frame), 0x0102_0304);
}
```

So the packed case collapses to two moves: copy the field out by value, or take
`&raw const` and use `read_unaligned` / `write_unaligned`. Reach for the pointer
form only when copying is genuinely not an option — an oversized field, or a
write that must land in place.

## See Also

- [serde-byte-order](serde-byte-order.md) - the encoding the decoded fields must agree on
- [unsafe-sound-abstractions](unsafe-sound-abstractions.md) - why the unchecked cast cannot sit behind a safe function
- [err-short-read](err-short-read.md) - decode only the bytes that arrived
- [mem-assert-type-size](mem-assert-type-size.md) - why in-memory layout is not a wire format
