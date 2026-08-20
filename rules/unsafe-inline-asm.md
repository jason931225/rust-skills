# unsafe-inline-asm

> Declare every register, memory, and flag effect an `asm!` block has; the options you pass are promises the optimizer acts on

## Why It Matters

`asm!` is unsafe for a reason most `unsafe` blocks are not: the compiler
cannot read the assembly, so it believes whatever the operand specifiers and
`options(...)` claim and optimizes around them. A block that writes a register
it declared only as an input, touches memory after promising `nomem`, or
clobbers the flags without saying so does not fail loudly — the compiler
schedules other code into those registers, hoists a load across the block, or
drops a comparison it thinks is still valid. The result is a miscompilation
that appears only at some optimization levels, on some targets, or after an
unrelated edit changes register allocation. Unlike a bad pointer dereference,
there is no address to inspect and nothing for a sanitizer to trap on.

## What Each Promise Commits To

- Declare every operand with the direction it is actually used:
  `in` for read-only, `out` for write-only, `inout` (or `lateout`) when the
  same register is both. Writing through something declared `in` is undefined
  behavior even when the value is never read again.
- List every register the assembly modifies but does not return through an
  operand in `clobber_abi(...)` or an explicit `out(reg) _`. A silently
  clobbered register is the classic cause of corruption in the *caller*.
- `options(nomem)` promises the block reads and writes no memory;
  `options(readonly)` promises it does not write. Either one lets the compiler
  keep values in registers across the block and reorder loads and stores
  around it. Do not pass them for a block that touches memory-mapped I/O or
  dereferences a pointer operand.
- `options(preserves_flags)` promises the condition flags are unchanged. Most
  arithmetic instructions modify them, so this is wrong more often than it
  looks; omit it unless the instruction genuinely leaves flags alone.
- `options(pure)` promises the block has no side effects and its output is a
  function of its inputs only — the compiler may then cache or delete calls.
  It requires `nomem` or `readonly`, and it is wrong for anything that
  observes time, randomness, or hardware state.
- `options(nostack)` promises the block does not push, pop, or write below the
  stack pointer, including the red zone on targets that have one.
- Gate the block on the architecture it is written for with
  `#[cfg(target_arch = ...)]`, and provide a portable path for every other
  target. `asm!` is not portable, and a missing arm is a build failure at
  best.
- Prefer an intrinsic or a `core::sync::atomic` operation when one exists;
  reach for `asm!` only when nothing in the standard library or a maintained
  intrinsics crate expresses the instruction you need.

## Bad

```rust
#[cfg(target_arch = "x86_64")]
pub fn double(mut value: u64) -> u64 {
    // Three separate lies to the optimizer:
    //   - `in(reg)` says the register is read-only, but `add` writes it
    //   - `nomem` is fine here, but `preserves_flags` is not: `add` sets them,
    //     so a comparison the compiler kept across this block is now wrong
    //   - nothing declares that the output actually comes back in that register
    unsafe {
        std::arch::asm!(
            "add {0}, {0}",
            in(reg) value,
            options(nomem, nostack, preserves_flags),
        );
    }
    value
}
```

## Good

```rust
/// Doubles `value` using one instruction, with every effect declared.
#[cfg(target_arch = "x86_64")]
pub fn double(mut value: u64) -> u64 {
    // SAFETY: the block reads and writes only the single `inout` operand.
    // It touches no memory, so `nomem` holds; it writes no stack, so `nostack`
    // holds. `add` *does* modify the condition flags, so `preserves_flags` is
    // deliberately not passed and the compiler will not keep flags across it.
    unsafe {
        std::arch::asm!(
            "add {0}, {0}",
            inout(reg) value,
            options(nomem, nostack),
        );
    }
    value
}

/// Every other target gets a portable implementation rather than a build error.
#[cfg(not(target_arch = "x86_64"))]
pub fn double(value: u64) -> u64 {
    value.wrapping_mul(2)
}

fn main() {
    assert_eq!(double(3), 6);
    assert_eq!(double(0), 0);
    // Wrapping is the documented behaviour of the portable path and matches
    // what the two's-complement `add` does on overflow.
    assert_eq!(double(u64::MAX), u64::MAX.wrapping_mul(2));
}
```

## Cases To Pin In Tests

- the assembly path and the portable path agree on the same inputs, including
  the overflow boundary — run the test on both a target that takes the `asm!`
  arm and one that does not;
- the function still behaves correctly at `opt-level=0` and at the level you
  ship; a wrong `options(...)` frequently shows up only when the optimizer
  acts on the promise;
- a value the caller holds across the call is unchanged, which is what catches
  an undeclared clobber;
- the block is exercised under Miri only if Miri supports it — Miri cannot
  execute `asm!`, so this path needs a real-hardware test rather than an
  interpreter run.

## See Also

- [unsafe-safety-comment](unsafe-safety-comment.md) - the local proof each `asm!` block owes, naming which promise holds and why
- [unsafe-volatile-mmio](unsafe-volatile-mmio.md) - the safe alternative for hardware registers, which `nomem` would break
- [conc-atomic-ordering](conc-atomic-ordering.md) - prefer a standard atomic to hand-written assembly for synchronisation
- [unsafe-miri-ci](unsafe-miri-ci.md) - why Miri cannot cover this path, and what has to cover it instead
- [proj-build-target-cfg](proj-build-target-cfg.md) - gating an architecture-specific path on the target rather than the host
