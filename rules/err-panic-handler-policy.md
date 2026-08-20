# err-panic-handler-policy

> A freestanding `#[panic_handler]` is a policy decision, not boilerplate: report if you can, then halt in a way that matches the device's power and recovery model

## Why It Matters

A `#![no_std]` binary has no runtime to catch a panic and no standard error
stream to print one on, so the language requires the program to supply
`#[panic_handler]` itself — the build fails outright without one. What goes in
that function is where the decision lives, and the shape everyone reaches for
first, `loop {}`, is the worst of the available options: it holds the core at
100% utilisation forever, which on a battery-powered device is a flat battery
and on any device is heat for no work. It is also silent, so a field failure
leaves nothing to diagnose. The handler runs at the moment the system is least
trustworthy, which constrains what it may safely do on the way out.

## Deciding What The Handler Does

- Report before halting if any output path exists at all — an RTT or `defmt`
  channel, semihosting, a UART, or even one status register or LED. A silent
  halt is indistinguishable from a hang, and `PanicInfo::location()` is
  usually the single most useful field you can emit.
- Choose the halt deliberately: a low-power wait instruction (`wfi`/`wfe` on
  ARM, `hlt` on x86) parks the core until an interrupt; letting a watchdog
  expire resets the device and is right when unattended recovery matters more
  than post-mortem state; `loop {}` is a busy-wait and should be the choice
  you argue for, not the one you default into.
- Do not allocate, format, or lock in the handler. The allocator may be
  exactly what failed, formatting machinery inflates a size-constrained
  binary, and a lock the panicking code already held will deadlock. Prefer a
  pre-formatted byte string or a numeric code.
- Set `panic = "abort"` in the profile. There is nothing to unwind to in a
  freestanding binary, and unwinding otherwise demands an `eh_personality`
  lang item whose empty implementation makes every fault fatal in a way that
  is harder to debug than aborting.
- Keep the handler total. Its signature returns `!`, so every path must
  diverge; a handler that could fall through is a compile error, and one that
  "returns" by resetting should say so.
- Do not confuse the two panic types: a `#[panic_handler]` receives
  `core::panic::PanicInfo`, while `std::panic::set_hook` receives
  `std::panic::PanicHookInfo` — separate types since Rust 1.81. Both expose
  `location()`, so code written against one will not compile against the
  other despite looking identical.
- Test the handler on hardware or an emulator. A host `cargo test` links the
  standard library and its own panic machinery, so it never exercises this
  code at all.

## Bad

```rust
// The shape this rule exists to argue against, shown as it would appear in a
// freestanding binary:
//
//   #[panic_handler]
//   fn panic(_info: &core::panic::PanicInfo) -> ! {
//       loop {}
//   }
//
// Three problems in four lines: the core spins at 100% until power is pulled,
// the `_info` binding throws away the only diagnostic the language handed us,
// and nothing distinguishes this state from an ordinary hang.
```

## Good

```rust
use core::panic::PanicInfo;

/// How the handler leaves the core once it has reported. The choice belongs
/// to the product: a sensor that must recover unattended resets, a debugger
/// session wants the core parked and inspectable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HaltPolicy {
    /// Park the core until an interrupt. Lowest power, keeps state for a
    /// debugger to inspect.
    WaitForInterrupt,
    /// Stop petting the watchdog and let it reset the device.
    ResetViaWatchdog,
    /// Busy-wait. Holds the core at 100%; choose it only deliberately.
    BusyLoop,
}

/// The one diagnostic worth emitting even when almost nothing is safe to do.
/// Returns the failure site rather than formatting it, so the handler does no
/// allocation and pulls in no formatting machinery.
pub fn failure_site(info: &PanicInfo<'_>) -> Option<(u32, u32)> {
    info.location().map(|site| (site.line(), site.column()))
}

// In the real binary the handler is a lang item that never returns:
//
//   #[panic_handler]
//   fn panic(info: &PanicInfo) -> ! {
//       if let Some((line, column)) = failure_site(info) {
//           report_code(line, column);          // pre-formatted, no alloc
//       }
//       match POLICY {
//           HaltPolicy::WaitForInterrupt => loop { cortex_m::asm::wfi() },
//           HaltPolicy::ResetViaWatchdog => loop { /* stop petting it */ },
//           HaltPolicy::BusyLoop         => loop {},
//       }
//   }

fn main() {
    // The policy is a real choice with distinct values, not a comment.
    assert_ne!(HaltPolicy::BusyLoop, HaltPolicy::WaitForInterrupt);

    // `failure_site` extracts the location without allocating or formatting.
    let caught = std::panic::catch_unwind(|| panic!("boom"));
    assert!(caught.is_err(), "the panic was observed");
}
```

## Cases To Pin In Tests

- the handler is exercised on hardware or an emulator, not only on the host —
  a host `cargo test` links `std` and its own panic machinery and never runs
  this code;
- a deliberate panic reaches the reporting path and emits the failure site
  before the halt, so a field failure is diagnosable;
- the chosen halt actually behaves as intended: measure current draw or core
  utilisation to tell a low-power wait from a busy loop, since both look like
  "stopped" from outside;
- the reset path, where one is chosen, brings the device back rather than
  wedging it — a watchdog that was disabled during the fault never fires;
- the handler allocates nothing, which is what keeps it working when the
  allocator is the component that failed.

## See Also

- [type-never-diverge](type-never-diverge.md) - the `!` return type the handler's signature requires
- [err-catch-unwind-boundary](err-catch-unwind-boundary.md) - the hosted counterpart, where unwinding exists and a boundary can catch it
- [test-cross-target-execution](test-cross-target-execution.md) - why a green host test says nothing about this code
- [unsafe-volatile-mmio](unsafe-volatile-mmio.md) - how the reporting path usually reaches a register or peripheral
- [perf-release-profile](perf-release-profile.md) - `panic = "abort"` as a profile decision rather than a size switch
