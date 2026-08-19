# test-sanitizers

> Run the tests that exercise unsafe, FFI, or concurrency under sanitizers in CI, and treat a report as a bug

## Why It Matters

Undefined behaviour and data races are not reliably observable: a test that
reads freed memory or races on a field usually passes, until layout, timing,
or an optimiser change makes it fail somewhere else. Sanitizers instrument the
compiled program so those events are detected when they happen rather than
when they finally corrupt something. AddressSanitizer finds use-after-free,
buffer overflow, and leaks; ThreadSanitizer flags unsynchronised concurrent
accesses. They cost a constant factor — roughly single-digit multiples of
runtime — so unlike an interpreter they can run realistic test cases.

## Bad

```yaml
# the unsafe ring buffer is only ever exercised by an optimised, uninstrumented
# test run, so use-after-free and races stay invisible until production
- run: cargo test --workspace --release
```

## Good

```bash
# Pinned nightly; scope to the crates whose tests exercise unsafe or threads.
RUSTFLAGS="-Zsanitizer=address" \
  cargo +nightly-2026-02-28 test -Zbuild-std --target x86_64-unknown-linux-gnu -p wire-codec

RUSTFLAGS="-Zsanitizer=thread" \
  cargo +nightly-2026-02-28 test -Zbuild-std --target x86_64-unknown-linux-gnu -p scheduler

cargo +nightly-2026-02-28 miri test -p wire-codec
```

Sanitizer support is target-specific; check the platform support table before
promising a job on a given runner.

## Key Points

- Run the unsafe, FFI, and concurrency test suites under AddressSanitizer and
  ThreadSanitizer on a pinned nightly toolchain (`-Zsanitizer=address`,
  `-Zsanitizer=thread`), and pin the toolchain so a report is reproducible.
- Read a clean run as evidence about the executions that ran, not as proof of
  soundness: sanitizers see only code the tests reach.
- Pair them with Miri, which catches Rust-level undefined behaviour that
  machine-code instrumentation cannot see, and with Loom for interleavings.
- Do not mix sanitizers in one run, and rebuild the standard library
  (`-Zbuild-std`) when the tool requires it; a partially instrumented binary
  reports misleading frames.
- Put assertions in unsafe and concurrent code so violated invariants become
  detectable events instead of silent corruption.
- Every report is a bug until proven otherwise. Add a regression test for each
  one, which makes the next run of these tools more effective.

## See Also

- [unsafe-miri-ci](unsafe-miri-ci.md) - the interpreter-based half of the same evidence
- [test-loom-concurrency](test-loom-concurrency.md) - exhaustive interleavings for small concurrent models
- [test-fuzz-target](test-fuzz-target.md) - generate the inputs the sanitizers then observe
- [lint-static-verification](lint-static-verification.md) - where these jobs sit in the CI gate
