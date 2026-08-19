# unsafe-miri-ci

> Run pinned Miri jobs over the unsafe paths Miri can execute, and read a clean run as evidence about those executions, not as a soundness proof

## Why It Matters

Miri interprets Rust at the MIR level and reports undefined behavior — out-of-bounds accesses, use-after-free, reads of uninitialized memory, invalid pointer provenance, misaligned accesses, aliasing violations under Stacked Borrows or Tree Borrows, and data races — on the executions it actually performs. It cannot interpret everything: native FFI calls, inline assembly, many syscalls, and some intrinsics either refuse to run or need flags that weaken isolation, and a single run explores one thread interleaving. A green Miri job therefore means the executed paths produced no detected UB; it does not prove the crate is sound, and unexecuted code is unchecked. Pair Miri with code review, sanitizer and real-platform runs, interleaving models where concurrency matters, and a written rationale wherever coverage is scoped down.

## Bad

```yaml
# .github/workflows/ci.yml — unsafe modules are compiled but never interpreted.
- name: Test
  run: cargo test --all-features

# .github/workflows/miri.yml — unreproducible and all-or-nothing.
- name: Miri
  run: |
    rustup toolchain install nightly --component miri   # floating nightly
    cargo miri test --all-features                      # includes FFI-backed tests
# When the FFI tests fail under Miri, the job gets deleted instead of scoped,
# and the remaining green badge is quoted as "the unsafe code is proven safe".
```

## Good

```yaml
# .github/workflows/miri.yml
name: miri

on:
  push:
    branches: [main]
  pull_request:

jobs:
  miri:
    runs-on: ubuntu-latest
    env:
      # Pinned on purpose: a floating nightly makes a failure unreproducible.
      MIRI_TOOLCHAIN: nightly-2026-07-01
      MIRIFLAGS: -Zmiri-strict-provenance
    steps:
      # Pinned to a commit: a moving tag changes what the job runs.
      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.3.0

      - name: Install pinned Miri toolchain
        run: |
          rustup toolchain install "$MIRI_TOOLCHAIN" --component miri
          cargo +"$MIRI_TOOLCHAIN" miri setup

      # Feature set chosen so every selected test is interpretable:
      # the `native-codec` feature links C code Miri cannot execute.
      # One module per step keeps a failure tied to that module in the job log;
      # libtest ORs multiple positional filters when they share an invocation.
      - name: Interpret the raw-pointer module
        run: |
          cargo +"$MIRI_TOOLCHAIN" miri test -p ring-buffer --lib \
            --no-default-features --features std \
            -- raw_ptr::

      - name: Interpret the ring module
        run: |
          cargo +"$MIRI_TOOLCHAIN" miri test -p ring-buffer --lib \
            --no-default-features --features std \
            -- ring::

      - name: Sweep schedules for the concurrent module
        run: |
          for seed in 0 1 2 3; do
            MIRIFLAGS="$MIRIFLAGS -Zmiri-seed=$seed" \
              cargo +"$MIRI_TOOLCHAIN" miri test -p ring-buffer --lib \
                --no-default-features --features std \
                -- shared::
          done
```

Record what this job does not cover — here, the `native-codec` FFI path and the
integration suite — next to the job, and cover those with the checks that can
actually run them.

## Key Points

- **Pinned nightly**: Miri ships with nightly. Pin the toolchain (workflow env or `rust-toolchain.toml`) so a Miri failure reproduces locally, and bump it as a reviewed change.
- **Scope, don't drop**: Miri interprets rather than compiles, so suites commonly run orders of magnitude slower. When cost or unsupported operations block the full suite, select the packages, features, and test filters that exercise the unsafe paths instead of removing the job.
- **What Miri cannot execute**: calls into native libraries, inline assembly, and many platform APIs. `-Zmiri-disable-isolation` unblocks clocks, randomness, and filesystem access at the price of determinism; prefer test-level fakes so the interpreted run stays repeatable.
- **Aliasing model**: Stacked Borrows is the default; `-Zmiri-tree-borrows` selects the newer model. They reject different programs, so state which one the job enforces.
- **Provenance**: `-Zmiri-strict-provenance` rejects the int-to-pointer casts that the permissive fallback allows — useful for raw-pointer and pointer-tagging code.
- **Concurrency**: Miri checks the interleaving it ran. Vary `-Zmiri-seed` for cheap schedule diversity, and use `loom` when a synchronization protocol needs systematic exploration.
- **Cold start**: `cargo miri setup` prebuilds the sysroot; run it once per cache key.
- **Dependencies**: Miri also interprets dependency code reached by the selected tests, so an upstream UB report can appear in a crate you do not own; triage it upstream rather than muting the job.

## Scoping Down Without Faking Coverage

Each reduction below is legitimate only with a recorded reason and a compensating check:

- **No `unsafe` reached by the selected tests**: the marginal value is low; revisit when unsafe code or a dependency with unsafe internals is added.
- **FFI, inline asm, or platform APIs Miri cannot execute**: exclude those tests by feature or filter, and verify them with real-target tests and sanitizer runs (ASan/TSan/UBSan).
- **Full suite is too slow**: run a targeted unit-test subset per pull request and a broader nightly job, and state which paths only the nightly job touches.
- **Generated or proc-macro output you do not control**: audit the generator and test the generated behavior where it runs.

## See Also

- [unsafe-safety-comment](unsafe-safety-comment.md) - document the local proof for every unsafe block
- [unsafe-sound-abstractions](unsafe-sound-abstractions.md) - never expose a safe API that can reach UB
- [unsafe-maybeuninit](unsafe-maybeuninit.md) - use `MaybeUninit<T>` for uninitialized memory
- [test-loom-concurrency](test-loom-concurrency.md) - explore interleavings Miri's single run does not
- [lint-static-verification](lint-static-verification.md) - gate the rest of the verification suite in CI
