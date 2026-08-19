# test-fuzz-target

> Fuzz every parser and decoder that touches untrusted bytes, and keep crashers as regression tests

## Why It Matters

A test suite only covers the cases someone thought of. Coverage-guided fuzzing
generates inputs and uses execution feedback to reach paths hand-written tests
never visit, which is exactly what an attacker's input space looks like. The
target does not need to know the right answer — it looks for panics, hangs,
and violated invariants, so any function that turns bytes into a structure can
be fuzzed with a few lines. For code with an unsafe or arithmetic-heavy core,
a fuzzer plus a sanitizer finds the bugs that a review will not.

## Contract

- Fuzz every boundary decoder: wire formats, file formats, query and header
  parsing, decompression, and deserialization of untrusted input.
- Keep targets total: pass bytes in, ignore expected `Err` results, and let the
  fuzzer look for panics, aborts, and timeouts.
- Use `arbitrary` to turn raw bytes into structured inputs, and drive a
  sequence of operations through an `Operation` enum when the bug you fear is
  stateful rather than single-call.
- Assert real invariants inside the target — round-trip equality, or agreement
  with a slow reference implementation — so the fuzzer can find wrong answers,
  not only crashes.
- Check in the seed corpus and every minimized crasher as an ordinary
  regression test, so a fixed bug stays fixed.
- Run fuzzing on a schedule with a time budget in CI; an unbounded fuzz job
  never finishes on its own.

## Bad

```rust
#[test]
fn parses_a_header() {
    // one hand-picked input; the malformed space is never explored
    assert!(parse_header(b"content-length: 12").is_ok());
}
```

## Good

```rust
/// Total parser: every byte string either parses or returns an error.
pub fn parse_header(input: &[u8]) -> Result<(&[u8], &[u8]), ()> {
    let colon = input.iter().position(|byte| *byte == b':').ok_or(())?;
    let (name, rest) = input.split_at(colon);
    let value = rest.get(1..).ok_or(())?;
    if name.is_empty() {
        return Err(());
    }
    Ok((name, value))
}

/// The property a fuzz target asserts: parsing never panics, and a successful
/// parse accounts for every byte of the input.
pub fn parse_is_total_and_lossless(input: &[u8]) {
    if let Ok((name, value)) = parse_header(input) {
        assert_eq!(name.len() + 1 + value.len(), input.len());
    }
}

fn main() {
    // The same property, checked here against the seed corpus and past crashers.
    let corpus: [&[u8]; 6] = [b"a:b", b"", b":", b"x:", b"no-colon", &[0xff, b':', 0x00]];
    for seed in corpus {
        parse_is_total_and_lossless(seed);
    }
}
```

The `cargo fuzz` target is then a three-line wrapper that hands the fuzzer's
bytes to the same function the regression test calls:

```text
fuzz_target!(|data: &[u8]| { parse_is_total_and_lossless(data) });
```

Keeping the property in the crate — not in the fuzz harness — is what lets the
corpus double as a normal test.

## See Also

- [test-proptest-properties](test-proptest-properties.md) - property testing states the expected answer; fuzzing hunts for crashes
- [test-sanitizers](test-sanitizers.md) - run fuzz targets under a sanitizer to catch silent corruption
- [unsafe-miri-ci](unsafe-miri-ci.md) - Miri checks the executions your tests reach
- [api-parse-dont-validate](api-parse-dont-validate.md) - the parsers worth fuzzing are the ones at the boundary
- [api-resource-limits](api-resource-limits.md) - a hang on crafted input is a denial of service
