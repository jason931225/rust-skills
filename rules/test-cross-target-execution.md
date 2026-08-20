# test-cross-target-execution

> A green `cargo test` proves the host build works; a target you cross-compile for is untested until something runs the binary there

## Why It Matters

`cargo test` builds and runs test binaries for the *host*, and two common
setups make that quietly different from the thing being shipped. Cross-compiling
to another triple produces a binary the host cannot execute, so `cargo test
--target <triple>` builds it and then fails to run it — or, worse, a CI job
runs `cargo test` without the flag, tests the host build, and reports green for
a target it never exercised. A `#![no_std]` library has the same gap for a
different reason: the test harness itself links `std`, so `cargo test --lib`
compiles the crate against the host's `std` and runs there, proving nothing
about whether it builds for a `-none-` target at all. In both cases the failing
configuration is the one nobody executed, and the signal that would have caught
it is a passing test suite.

## Making The Target Actually Run

- Treat "compiles for the target" and "passes tests on the target" as two
  separate CI facts, and produce both. `cargo build --target <triple>` gives
  the first; only executing the binary gives the second.
- Configure a runner for targets the host cannot execute —
  `[target.<triple>] runner = "qemu-<arch> -L /usr/<arch>-linux-gnu"` or the
  equivalent — so `cargo test --target <triple>` actually runs the binary
  instead of failing at exec, or run it on real hardware.
- When tests run inside a cross-compilation container, mount the fixtures they
  read; that environment is not the workspace, and a test that opens a path
  relative to the repository root will not find it.
- For a `no_std` library, add a build-only job that compiles for a real
  bare-metal or embedded triple. Host `cargo test --lib` passing says nothing
  about `no_std` correctness, because the harness pulled in `std` to run at all.
- Keep the two kinds of test separate in the crate: logic that can be verified
  on the host (pure functions, parsers, state machines) and behavior that
  genuinely needs the target. Put the first in ordinary tests so they stay
  fast, and do not let their passing stand in for the second.
- Where an artifact is promoted between stages, execute the *same* binary that
  was built rather than rebuilding on the runner — a rebuild on a different
  machine is a different artifact and re-opens the gap this rule closes.

## Bad

```toml
# CI: the only test step. Builds and runs for the *host*, then reports green
# for a project whose shipped artifact is aarch64 and whose library claims
# no_std support. Neither of those was executed, or even compiled.
#
#   - run: cargo test --all-features
```

## Good

```toml
# Three distinct facts, each produced by a step that can actually fail.
#
#   # 1. host logic — fast, runs everywhere
#   - run: cargo test --all-features
#
#   # 2. the shipped target executes its own tests, via a runner
#   - run: cargo test --target aarch64-unknown-linux-gnu
#     # .cargo/config.toml supplies:
#     #   [target.aarch64-unknown-linux-gnu]
#     #   runner = "qemu-aarch64 -L /usr/aarch64-linux-gnu"
#
#   # 3. no_std support is a build fact; the host harness cannot prove it
#   - run: cargo build --no-default-features --target thumbv7em-none-eabihf
```

```rust
/// Logic that can be verified on the host, kept separate from anything that
/// needs the target so a fast host run does not masquerade as target coverage.
pub fn frame_len(header: &[u8]) -> Option<usize> {
    let raw = u16::from_le_bytes([*header.first()?, *header.get(1)?]);
    Some(usize::from(raw))
}

fn main() {
    assert_eq!(frame_len(&[0x04, 0x00]), Some(4));
    assert_eq!(frame_len(&[0x00, 0x01]), Some(256));
    assert_eq!(frame_len(&[0x04]), None, "a short header has no length");
}
```

## Cases To Pin In Tests

- the target test step fails loudly when no runner is configured, rather than
  being skipped or reported as passing;
- a deliberate target-only failure (an assertion that holds on the host and
  not on the target, or vice versa) is caught by the target job and missed by
  the host job — proving the two steps are genuinely different;
- the `no_std` build job fails when a `std`-only dependency is introduced,
  even though `cargo test --lib` still passes on the host;
- fixtures a cross-container test reads are present inside that container, not
  merely in the workspace;
- the binary executed in a promotion step is byte-identical to the one built
  in the earlier stage.

## See Also

- [proj-build-target-cfg](proj-build-target-cfg.md) - reading the target rather than the host when the build makes decisions
- [proj-libc-floor](proj-libc-floor.md) - the other way a target-specific artifact fails only after deployment
- [test-env-independent](test-env-independent.md) - separating what the program decides from what the host decides
- [proj-reproducible-runtime](proj-reproducible-runtime.md) - promoting the exact artifact rather than rebuilding per stage
- [proj-feature-additive](proj-feature-additive.md) - why a `no_std` baseline is an additive `std` feature, not a subtractive one
