# serde-format-version

> Start every persisted binary format with a magic identifier and a version, and reject versions you do not understand

## Why It Matters

A positional binary encoding has no field names, so nothing about the bytes
says which layout produced them. Add a field, reorder two, or widen an integer
and yesterday's file decodes into today's struct without complaint —
misaligned, not rejected. Named formats survive this because a decoder can
match keys; `bincode` and hand-rolled records cannot. A magic identifier tells
a decoder the file belongs to this format at all, and a version says which
reader applies.

## Bad

```rust
fn load(path: &Path) -> io::Result<Index> {
    // Any file at all decodes into something; a v1 file read by a v2 binary
    // silently produces wrong offsets
    let bytes = fs::read(path)?;
    Ok(bincode::deserialize(&bytes).unwrap())
}
```

## Good

```rust
use std::io::{self, Read, Write};

const MAGIC: [u8; 4] = *b"IDX1"; // identifies the format, not the version
const VERSION: u16 = 2; // bumped whenever the record layout changes

#[derive(Debug, PartialEq)]
pub enum FormatError {
    NotOurFormat,
    UnsupportedVersion(u16),
    Truncated,
}

pub fn write_header(sink: &mut impl Write) -> io::Result<()> {
    sink.write_all(&MAGIC)?;
    sink.write_all(&VERSION.to_be_bytes())
}

pub fn read_header(source: &mut impl Read) -> Result<u16, FormatError> {
    let mut magic = [0u8; 4];
    let mut version = [0u8; 2];
    source.read_exact(&mut magic).map_err(|_| FormatError::Truncated)?;
    source.read_exact(&mut version).map_err(|_| FormatError::Truncated)?;
    if magic != MAGIC {
        return Err(FormatError::NotOurFormat);
    }
    match u16::from_be_bytes(version) {
        // Older versions are read through an explicit migration, never by
        // pointing the current decoder at them.
        supported @ 1..=VERSION => Ok(supported),
        other => Err(FormatError::UnsupportedVersion(other)),
    }
}

fn main() {
    let mut file = Vec::new();
    write_header(&mut file).expect("write");
    assert_eq!(read_header(&mut &file[..]), Ok(VERSION));

    assert_eq!(read_header(&mut &b"JPEG\0\x01"[..]), Err(FormatError::NotOurFormat));
    assert_eq!(read_header(&mut &b"IDX1\0\x63"[..]), Err(FormatError::UnsupportedVersion(99)));
    assert_eq!(read_header(&mut &b"IDX"[..]), Err(FormatError::Truncated));
}
```

## Version Bumps And Migrations

- Bump the version for any layout change, including widening an integer or
  adding a field — a positional format has no compatible additions.
- Refuse unknown versions explicitly. A newer file read by an older binary is
  the case that corrupts data silently.
- Keep migrations as separate decoders that produce the current type, rather
  than conditionals threaded through one decoder.
- Self-describing formats (JSON, TOML) get their compatibility from field
  names instead; this rule is for positional encodings.
- The magic identifier is not security — pair it with an integrity check when
  the bytes can be tampered with.

## See Also

- [serde-byte-order](serde-byte-order.md) - fix the encoding as well as the layout
- [api-record-checksum](api-record-checksum.md) - detect corruption the header cannot
- [proj-schema-migrations](proj-schema-migrations.md) - the same discipline for database schemas
- [api-parse-dont-validate](api-parse-dont-validate.md) - decode into a type that states what was verified
