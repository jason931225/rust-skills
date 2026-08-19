# proj-append-log-recovery

> Make a truncated trailing record a clean end of log, and a malformed interior record a loud failure

## Why It Matters

An append-only log is the one file format guaranteed to be interrupted: a crash
or a power loss lands mid-write, leaving a final record that is short by a few
bytes. Recovery has to tell that ordinary case apart from real corruption. A
loader that treats every read failure as fatal refuses to start after any
unclean shutdown; one that stops at the first problem silently discards
everything after a bit flip. The distinction is structural — a short read at
the very end is expected, anything else is not — and it has to be decided in
the loader, not left to whichever error surfaces first.

## Contract

- Reaching end of file exactly at a record boundary ends the scan successfully.
- A record header that is short at the end of the file ends the scan as a
  partial write; record how many bytes were discarded.
- A record whose length or checksum is wrong *before* the end of the file is
  corruption: fail loudly, and do not silently truncate the log at that point.
- Verify each record's integrity check before its payload is used or indexed.
- Recover into a fresh index rather than mutating the live one, so a failed
  load leaves the previous state intact.
- Record the recovered length; append from there so a partial tail is
  overwritten rather than kept.
- Make replay idempotent: recovery may run twice after a crash during recovery.

## Bad

```rust
fn load(file: &mut File, index: &mut HashMap<Key, u64>) -> io::Result<()> {
    loop {
        // Any error ends the loop, so a bit flip in the middle silently
        // discards every record after it — and a clean EOF looks the same
        let Ok(record) = read_record(file) else { return Ok(()) };
        index.insert(record.key, record.position);
    }
}
```

## Good

```rust
use std::collections::HashMap;

#[derive(Debug, PartialEq)]
pub enum Recovery {
    /// Scan ended on a record boundary.
    Clean { records: usize },
    /// A short record at the end: an interrupted append.
    PartialTail { records: usize, discarded: usize },
}

#[derive(Debug, PartialEq)]
pub enum Corruption {
    /// A bad record with more data after it cannot be an interrupted write.
    Interior { offset: usize },
}

const HEADER: usize = 2; // [len, checksum] for the example

fn checksum(payload: &[u8]) -> u8 {
    payload.iter().fold(0u8, |acc, byte| acc ^ byte)
}

pub fn recover(log: &[u8]) -> Result<(Recovery, HashMap<usize, usize>), Corruption> {
    let mut index = HashMap::new();
    let mut offset = 0;
    let mut records = 0;

    while offset < log.len() {
        let rest = &log[offset..];
        // Too little left for a header or a payload: an interrupted append.
        if rest.len() < HEADER || rest.len() < HEADER + usize::from(rest[0]) {
            return Ok((
                Recovery::PartialTail { records, discarded: rest.len() },
                index,
            ));
        }
        let len = usize::from(rest[0]);
        let payload = &rest[HEADER..HEADER + len];
        if checksum(payload) != rest[1] {
            // There are more bytes after this record, so it is not a torn
            // tail — the log is damaged and the caller must know.
            return Err(Corruption::Interior { offset });
        }
        index.insert(records, offset);
        records += 1;
        offset += HEADER + len;
    }
    Ok((Recovery::Clean { records }, index))
}

fn main() {
    let record = |payload: &[u8]| {
        let mut out = vec![payload.len() as u8, checksum(payload)];
        out.extend_from_slice(payload);
        out
    };

    let mut log = record(b"one");
    log.extend(record(b"two"));
    assert_eq!(recover(&log).expect("clean").0, Recovery::Clean { records: 2 });

    // Interrupted append: the last record is short.
    let mut torn = log.clone();
    torn.extend(record(b"three")[..4].to_vec());
    assert_eq!(
        recover(&torn).expect("partial").0,
        Recovery::PartialTail { records: 2, discarded: 4 }
    );

    // A flipped bit in the first record, with data after it, is corruption.
    let mut damaged = log.clone();
    damaged[2] ^= 0b1;
    assert_eq!(recover(&damaged), Err(Corruption::Interior { offset: 0 }));
}
```

## Failure Tests

- a log ending exactly on a boundary recovers clean;
- a log with a short trailing header, and one with a short trailing payload,
  both recover as a partial tail with the discarded byte count;
- a corrupted record followed by more data fails rather than truncating;
- appending after recovery overwrites the discarded tail;
- running recovery twice yields the same index.

## See Also

- [api-record-checksum](api-record-checksum.md) - the per-record check this scan relies on
- [err-short-read](err-short-read.md) - a short read at the tail is the expected case
- [serde-format-version](serde-format-version.md) - the header that says how to read the records
- [async-tokio-fs](async-tokio-fs.md) - the durability contract for the appends themselves
- [conc-db-transaction-boundary](conc-db-transaction-boundary.md) - the alternative when a store must be transactional
