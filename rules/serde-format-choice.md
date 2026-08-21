# serde-format-choice

> Choose the encoding from who reads the bytes — self-describing and editable for people, positional for same-binary data, a cross-language binary format where another runtime decodes

## Why It Matters

Every other rule here decides how to encode once a format is fixed, which
quietly assumes somebody chose one. The choice is usually inherited from the
first example that worked, and it decides things that are expensive to revisit:
whether a human can fix a value in an editor, whether an unknown field is a
warning or a parse failure, whether the payload survives the next release, and
whether the decoder can exist at all on the target that has to read it.

## Bad

```rust
// JSON because it is what the tutorial used — for a cache entry that only this
// binary writes and reads, at high volume. Every record now carries its field
// names, and the format's flexibility is spent on a reader that is this same
// program.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct CacheEntry {
    pub key: String,
    pub fetched_at_ms: u64,
    pub payload: Vec<u8>,
}
```

## Good

```rust
// The audience decides the encoding, so state the audience in the type's docs.

/// Operator-edited. Self-describing, diffable, commented — a person has to be
/// able to open this and change one value.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ServiceConfig {
    pub listen: String,
    pub workers: usize,
}

/// Written and read only by this binary, at volume, in a positional format:
/// no field names on the wire. Because a positional encoding has no names to
/// reconcile against, the layout carries its own version — without it a record
/// written by the previous release decodes into whatever the current struct
/// happens to be.
pub const CACHE_FORMAT_VERSION: u16 = 1;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct CacheEntry {
    /// First field on the wire, and checked before the rest is trusted.
    pub format_version: u16,
    pub key: String,
    pub fetched_at_ms: u64,
    pub payload: Vec<u8>,
}

impl CacheEntry {
    pub fn decoded(self) -> Result<Self, &'static str> {
        if self.format_version != CACHE_FORMAT_VERSION {
            return Err("cache record written by a different format version");
        }
        Ok(self)
    }
}

fn main() {
    let entry = CacheEntry {
        format_version: CACHE_FORMAT_VERSION,
        key: "k".to_string(),
        fetched_at_ms: 0,
        payload: Vec::new(),
    };
    assert!(entry.decoded().is_ok());

    let stale = CacheEntry {
        format_version: CACHE_FORMAT_VERSION + 1,
        key: "k".to_string(),
        fetched_at_ms: 0,
        payload: Vec::new(),
    };
    assert!(stale.decoded().is_err(), "a version it does not understand is rejected");
}
```

## Matching The Encoding To Who Decodes It

- **A person edits it.** Configuration, fixtures, checked-in data. Wants a
  self-describing text format with comments and stable diffs. The cost is size
  and parse time, which does not matter for something read once at startup.
- **This binary wrote it and this binary reads it.** Caches, spool files,
  same-language IPC. A positional binary format drops the field names, which
  are pure overhead when both ends share the struct definition. The cost is
  that the bytes mean nothing without the code, so they need a version.
- **Another runtime decodes it.** A service in another language, a stored
  record with a long life. Wants a format with an independent specification
  and implementations elsewhere — self-describing binary, or a schema language
  if the contract is worth declaring separately.
- **The decoder is constrained.** `no_std`, a microcontroller, a hot path with
  no allocator. Wants a format whose decoder does not require allocation, which
  rules out most of the convenient ones.

## What The Choice Then Commits You To

The format is not an isolated decision; several other rules only apply on one
side of it.

- A positional format has no field names to reconcile, so a layout change is
  undetectable without a magic identifier and a version — see
  [serde-format-version](serde-format-version.md). A self-describing format
  gets that reconciliation from the names, and needs
  [serde-default-compat](serde-default-compat.md) instead.
- `flatten` and `deny_unknown_fields` need a self-describing format. On a
  positional one, `deny_unknown_fields` goes quietly inert — there are no field
  names to be unknown — while `flatten` usually fails loudly at encode time,
  because the encoder cannot size a map it has not seen. One of the two tells
  you; the other does not.
- Anything hand-rolled owes an explicit byte order regardless of the choice —
  see [serde-byte-order](serde-byte-order.md).

Where two audiences genuinely both exist, encode twice from one type rather
than compromising on a format that serves neither well. That costs a second
`derive` and keeps each side's constraints intact.

## See Also

- [serde-format-version](serde-format-version.md) - the versioning a positional format cannot do without
- [serde-default-compat](serde-default-compat.md) - how a self-describing format absorbs added fields
- [serde-enum-representation](serde-enum-representation.md) - a tagging choice the format constrains
- [coll-map-choice](coll-map-choice.md) - the same shape of decision for collections
- [api-record-checksum](api-record-checksum.md) - integrity for anything persisted or transmitted
