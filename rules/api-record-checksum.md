# api-record-checksum

> Store an integrity check with every persisted or transmitted record and verify it before trusting the bytes

## Why It Matters

Storage and transport corrupt data in ways the type system cannot see: a
partial write during a crash, a torn sector, a truncated upload, a bit flipped
in transit or in memory. Deserialization does not catch it — a corrupted
length still decodes as a length, and a flipped byte in a payload is simply a
different payload. A checksum written beside the record turns silent
corruption into a detected error at the point of reading, which is the only
place it can still be handled.

## Integrity Check Requirements

- Compute the check over exactly the bytes it protects, and define that span
  precisely (payload only, or header plus payload).
- Verify before decoding, and treat a mismatch as a typed error — never log
  and continue with the bytes.
- Say what a mismatch means for the caller: skip the record, fail the batch,
  or fall back to a replica. Silence is not a policy.
- Use a checksum for accidental corruption (CRC-32, xxHash) and a keyed MAC
  when an attacker may edit the bytes; a plain checksum is not authentication.
- Keep the algorithm and its width in the format's versioned header so it can
  be changed later.
- Cover the length prefix too, or a corrupted length can consume the next
  record before its own check is ever read.

## Bad

```rust
fn read_record(file: &mut File) -> io::Result<Record> {
    let len = read_len(file)?;
    let mut payload = vec![0u8; len];
    file.read_exact(&mut payload)?;
    // A torn write during the last crash decodes here as valid data
    Ok(Record::decode(&payload))
}
```

## Good

```rust
#[derive(Debug, PartialEq)]
pub enum RecordError {
    Corrupt { expected: u32, actual: u32 },
}

/// Stand-in for a real CRC or xxHash; the contract is the check, not the
/// polynomial.
fn checksum(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c_9dc5_u32, |acc, byte| {
        (acc ^ u32::from(*byte)).wrapping_mul(0x0100_0193)
    })
}

pub fn encode(payload: &[u8]) -> Vec<u8> {
    let mut record = checksum(payload).to_be_bytes().to_vec();
    record.extend_from_slice(payload);
    record
}

/// Verifies before the payload is handed to a decoder.
pub fn decode(record: &[u8]) -> Result<&[u8], RecordError> {
    let (stored, payload) = record.split_at(4);
    let expected = u32::from_be_bytes(stored.try_into().unwrap_or([0; 4]));
    let actual = checksum(payload);
    if expected != actual {
        return Err(RecordError::Corrupt { expected, actual });
    }
    Ok(payload)
}

fn main() {
    let record = encode(b"ledger entry");
    assert_eq!(decode(&record), Ok(&b"ledger entry"[..]));

    // One flipped bit is detected instead of decoded.
    let mut corrupted = record.clone();
    corrupted[6] ^= 0b0000_0001;
    assert!(matches!(decode(&corrupted), Err(RecordError::Corrupt { .. })));
}
```

## Corruption Cases To Test

- a single flipped bit anywhere in the protected span is rejected;
- a truncated record is rejected rather than decoded short;
- a record whose stored check was itself corrupted is rejected;
- the documented recovery action happens on mismatch, and the bad record does
  not reach the decoder;
- a mismatch is counted in telemetry — silent corruption that is merely
  skipped is still data loss.

## See Also

- [serde-format-version](serde-format-version.md) - the header that says which algorithm applies
- [api-crypto-primitives](api-crypto-primitives.md) - use a MAC when the threat is tampering, not noise
- [err-short-read](err-short-read.md) - verify only the bytes that actually arrived
- [async-tokio-fs](async-tokio-fs.md) - durability of the write this check protects
- [obs-operational-signals](obs-operational-signals.md) - corruption needs a signal, not a log line
