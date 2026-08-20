# type-line-terminator-fidelity

> Re-emit a line's original terminator and byte content when the tool's contract is byte-for-byte fidelity, not `BufRead::lines()` plus `println!`

## Why It Matters

`BufRead::lines()` strips both `\n` and a preceding `\r` from every line it
yields, and it fabricates nothing about what the source file actually had at
the end. A loop that reads with `lines()` and writes each line back with
`println!` normalizes every terminator to the host's `\n`, silently rewrites
a CRLF input to LF, and adds a trailing newline to a file that never had one.
For a text-processing tool whose contract is "reproduce these bytes," that
normalization is not a formatting nicety — it is the bug, and it is invisible
against any test fixture that happens to already use LF-only line endings
with a final newline. The fix is symmetric with how the bytes arrived:
`read_line`/`read_until` into a reused buffer keeps the exact terminator (or
its absence) attached, and `print!` (not `println!`) writes exactly what was
read.

## Byte-Faithful Line Handling

- When the contract is byte-for-byte reproduction (a filter, a line-numbering
  tool, anything meant to match a reference implementation's output), read
  with `read_line`/`read_until` and write with `print!`/`write_all`, not
  `lines()` plus `println!`.
- Track whether the source's final line had a trailing terminator, and match
  that in the output — do not add one that was not there, and do not drop one
  that was.
- Treat CRLF as data to preserve, not noise to normalize, unless the tool's
  documented contract is specifically to convert line endings.
- Reuse one buffer across lines (`clear()` between reads) instead of
  allocating a new `String` per line, matching the read side to the same
  performance discipline as the write side.
- Test with at least three fixtures: LF-terminated, CRLF-terminated, and a
  file whose last line has no terminator at all — a version built on
  `lines()` passes only the first.
- Reach for `lines()` freely everywhere the terminator is genuinely
  irrelevant (config parsing, log tailing for display); this rule is about
  code whose job is specifically to preserve what it read.

## Bad

```rust
use std::io::{BufRead, BufReader};

/// Looks like a faithful pass-through, but `lines()` strips every `\n` and
/// a preceding `\r`, and `println!` writes back only `\n` — a CRLF input
/// becomes LF, and a file with no final newline gains one it never had.
fn copy_lines(input: impl BufRead) -> std::io::Result<Vec<String>> {
    let mut out = Vec::new();
    for line in input.lines() {
        out.push(line?);
    }
    Ok(out)
}
```

## Good

```rust
use std::io::{BufRead, Write};

/// Reads and re-emits each line's exact bytes, including whatever
/// terminator (`\n`, `\r\n`, or none, on the final line) was actually there.
fn copy_lines_faithfully(mut input: impl BufRead, output: &mut impl Write) -> std::io::Result<()> {
    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        let read = input.read_until(b'\n', &mut buffer)?;
        if read == 0 {
            break; // clean EOF
        }
        output.write_all(&buffer)?;
    }
    Ok(())
}

fn main() {
    let crlf_input = b"one\r\ntwo\r\n".as_slice();
    let mut out = Vec::new();
    copy_lines_faithfully(crlf_input, &mut out).expect("copies cleanly");
    assert_eq!(out, b"one\r\ntwo\r\n", "CRLF terminators survive unchanged");

    let no_final_newline = b"one\ntwo".as_slice();
    let mut out2 = Vec::new();
    copy_lines_faithfully(no_final_newline, &mut out2).expect("copies cleanly");
    assert_eq!(
        out2, b"one\ntwo",
        "a missing final terminator is not fabricated"
    );
}
```

## Terminator Cases To Test

- a CRLF-terminated input round-trips with `\r\n` intact, not rewritten to
  `\n`;
- a file whose last line has no terminator produces output with no
  terminator on that line either;
- an all-LF fixture with a trailing newline round-trips unchanged (the
  common case still works);
- a version built on `lines()` + `println!` fails the CRLF and
  missing-final-newline cases above, which is exactly the regression this
  rule exists to catch.

## See Also

- [type-text-decode-policy](type-text-decode-policy.md) - the read-side decode decision this rule assumes has already been made
- [err-short-read](err-short-read.md) - trust the byte count `read_until` returns, the same way
- [perf-io-buffering](perf-io-buffering.md) - the buffered reader/writer this rule's loop runs inside
- [mem-reuse-collections](mem-reuse-collections.md) - clearing and reusing the line buffer instead of allocating one per line
- [test-cli-blackbox](test-cli-blackbox.md) - the golden-file test that actually catches a terminator regression
