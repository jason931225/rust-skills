# type-text-decode-policy

> Decide at the byte-to-text boundary whether invalid encoding is an error or a replacement, and make the choice visible

## Why It Matters

`String::from_utf8` and `String::from_utf8_lossy` answer different questions,
and reaching for whichever compiles hides the decision. Lossy decoding silently
substitutes U+FFFD, so a corrupted record, a mis-declared encoding, or a
truncated multi-byte sequence becomes ordinary-looking text that flows into a
database or a hash. Strict decoding turns the same input into an error the
caller must handle. Both are legitimate; picking one by accident is not.

## Bad

```rust
fn read_config(path: &Path) -> String {
    let bytes = fs::read(path).unwrap();
    // Corruption becomes replacement characters and is stored as if it parsed
    String::from_utf8_lossy(&bytes).into_owned()
}
```

## Good

```rust
use std::borrow::Cow;

#[derive(Debug, PartialEq)]
pub enum DecodeError {
    /// Byte offset of the first invalid sequence, so the caller can report it.
    Invalid { at: usize },
}

/// Data that must round-trip — identifiers, keys, stored records — decodes
/// strictly, and invalid input is a value the caller handles.
pub fn decode_exact(bytes: &[u8]) -> Result<&str, DecodeError> {
    std::str::from_utf8(bytes).map_err(|error| DecodeError::Invalid {
        at: error.valid_up_to(),
    })
}

/// Text that only reaches a human may be repaired, and the caller can see
/// whether it was.
pub fn decode_for_display(bytes: &[u8]) -> (Cow<'_, str>, bool) {
    let text = String::from_utf8_lossy(bytes);
    let repaired = matches!(text, Cow::Owned(_));
    (text, repaired)
}

fn main() {
    let valid = "ok".as_bytes();
    let invalid = &[0x66, 0xff, 0x6f];

    assert_eq!(decode_exact(valid), Ok("ok"));
    assert_eq!(decode_exact(invalid), Err(DecodeError::Invalid { at: 1 }));

    let (text, repaired) = decode_for_display(invalid);
    assert!(repaired, "the caller can tell the text was altered");
    assert!(text.contains('\u{fffd}'));

    let (text, repaired) = decode_for_display(valid);
    assert!(!repaired);
    assert_eq!(text, "ok");
}
```

## Strict Versus Lossy Decoding

- Strict decoding for anything compared, stored, hashed, or sent onward;
  lossy only for output a person reads.
- When decoding lossily, record that a substitution happened. Silent repair
  destroys the evidence that the input was broken.
- `valid_up_to()` gives the offset of the first bad byte — report it instead of
  a bare "invalid UTF-8".
- Bytes from a foreign encoding need a real decoder, not a lossy UTF-8 pass;
  U+FFFD is not a transcoding strategy.
- Filesystem paths and process arguments are OS strings, not UTF-8, and have
  their own rule.
- A wire protocol's line terminator and framing bytes are part of the
  protocol, not the host's text convention: write `b"\r\n\r\n"` or the
  protocol's literal bytes, not `writeln!` or a `&str` built with `\n` — a
  platform newline is not the same bytes as CRLF, and a `&str` also commits
  to an encoding the protocol may not share.

## See Also

- [type-path-not-string](type-path-not-string.md) - paths are not text and need no decoding
- [type-unicode-length](type-unicode-length.md) - once decoded, say what a length counts
- [api-parse-dont-validate](api-parse-dont-validate.md) - decode once at the boundary into a type
- [err-short-read](err-short-read.md) - truncation is a common source of invalid sequences
