# conc-signal-handler-safety

> Let a signal handler set an atomic flag and return; do everything else in ordinary code

## Why It Matters

A signal handler runs by interrupting whatever the process was doing, possibly
mid-allocation, mid-lock, or mid-`printf`. Only async-signal-safe operations
are legal there: allocating, taking a mutex, formatting a string, or logging
can deadlock against the very state the interrupted code was holding, and the
deadlock reproduces only under the timing that caused it. Restricting the
handler to one atomic store keeps the dangerous window to a single instruction
and moves the real work into code that can allocate, log, and fail properly.

## Bad

```rust
extern "C" fn on_term(_sig: i32) {
    // Allocates, takes the stdout lock, and may run inside the allocator that
    // the interrupted thread was already inside
    let report = format!("shutting down at {:?}", SystemTime::now());
    println!("{report}");
    database.close();      // locks, I/O, and Drop glue in a signal context
    std::process::exit(0); // runs atexit handlers from a handler
}
```

## Good

```rust
use std::sync::atomic::{AtomicBool, Ordering};

/// Written only by the handler, read by ordinary code.
static SHUTDOWN: AtomicBool = AtomicBool::new(false);

/// The entire handler: one atomic store. No allocation, no lock, no
/// formatting, no I/O, no `exit`.
extern "C" fn on_terminate(_signal: i32) {
    SHUTDOWN.store(true, Ordering::SeqCst);
}

/// The work happens here, where failure can be reported and Drop can run.
fn serve_until_shutdown(mut step: impl FnMut() -> bool) -> &'static str {
    while !SHUTDOWN.load(Ordering::SeqCst) {
        if !step() {
            return "finished";
        }
    }
    "shutdown requested"
}

fn main() {
    let mut ticks = 0;
    assert_eq!(
        serve_until_shutdown(|| {
            ticks += 1;
            ticks < 3
        }),
        "finished"
    );

    // Delivering the signal only flips the flag; the loop observes it.
    on_terminate(15);
    assert!(SHUTDOWN.load(Ordering::SeqCst));
    assert_eq!(serve_until_shutdown(|| true), "shutdown requested");
}
```

## Platform Signal Caveats

- Registration is platform work: `sigaction` through a vetted crate, or a
  runtime's own listener. The contract above is about what the handler may do,
  whichever mechanism installs it.
- An `AtomicBool` in a `static` is the right shape; `static mut` requires
  `unsafe` at every access and is a data race waiting for a second handler.
- The loop has to reach a check. A long blocking call postpones shutdown until
  it returns, so give blocking work a deadline or use a self-pipe or `signalfd`
  so the wait itself becomes interruptible.
- An async runtime that exposes signals as a stream has already done this — use
  it rather than installing a raw handler beside it.
- `SIGKILL` and `SIGSTOP` cannot be caught at all; a grace period is the only
  protection against the follow-up kill.
- Register with `sigaction`, not the C89 `signal()` function: System V
  `signal()` semantics reset the disposition to the default *after* delivery,
  so a second copy of the same signal arriving while the handler runs — two
  rapid `SIGTERM`s is enough — kills the process instead of invoking the
  handler again. Re-registering from inside the handler is a racy workaround,
  not a fix.
- `SIGPIPE` kills a bare C program on a write to a closed pipe, but Rust's
  std sets it to `SIG_IGN` before `main`, so the write returns `EPIPE`
  instead (verified: the process survives and keeps running). The work is
  therefore handling that error, not installing a handler — see
  `proj-cli-contract` for treating `BrokenPipe` as a normal end of output.
  Code that re-enables the default disposition takes the C behaviour back. `SIG_IGN` and `SIG_DFL` (ignore, and restore the
  original default) are dispositions in their own right, not only something
  you replace with a custom function.
- Signal, interrupt, and language "exception" name different things: a signal
  is the OS notifying the process of an event and can often be ignored; an
  interrupt is a CPU/hardware event the core cannot decline to service; a
  Rust panic is neither. Reaching for the wrong API — `sigaction` for a
  hardware fault, or an interrupt vector for a peer request — is the usual
  symptom of conflating them.
- `SIGINT` is ordinarily a person at a terminal; `SIGTERM` is a peer asking
  for a graceful stop; `SIGHUP` traditionally means "the controlling
  terminal went away" and by daemon convention now means "reread
  configuration," not "crash."
- On Windows, install `SetConsoleCtrlHandler` instead of a POSIX signal
  handler; code that only calls `sigaction`/`signal` compiles on Windows and
  silently never fires there. The portable pattern is "OS callback sets an
  atomic flag, ordinary code polls it" — the callback registration is what
  changes per platform.
- Never use `setjmp`/`longjmp` (or the LLVM `sjlj` unwind mechanism) to leave
  a Rust stack frame: the jump teleports the instruction pointer without
  running any `Drop` implementations on the frames it skips, so held locks,
  open file descriptors, and RAII guards leak or stay locked. This is
  distinct from ordinary Rust unwinding, which does run `Drop`.

## See Also

- [async-cancellation-token](async-cancellation-token.md) - the drain sequence this flag starts
- [conc-atomic-ordering](conc-atomic-ordering.md) - choosing the ordering for the flag
- [proj-avoid-statics](proj-avoid-statics.md) - why the flag is the rare acceptable global
- [api-health-probes](api-health-probes.md) - failing readiness is the first step after the flag flips
