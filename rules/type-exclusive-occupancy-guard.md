# type-exclusive-occupancy-guard

> While a foreign agent (DMA, GPU, an in-flight I/O ring) owns a buffer, hold it behind a `!Send` guard whose only way back is a consuming `wait(self) -> T`

## Why It Matters

Handing a buffer's address to a DMA engine, a GPU command queue, or an
`io_uring` submission does not transfer Rust's notion of ownership — the CPU
side still holds a value it can read, write, or move to another thread while
hardware is concurrently reading or writing the same memory. That is a data
race the type system cannot see through an ordinary `&mut T` or a channel
send, because nothing about those types says "this value is not really
available right now." A guard type that owns the buffer for the duration of
the transfer, is not `Send`, and only gives the buffer back through a
consuming method closes both holes at once: the CPU cannot alias the memory
while it is in flight (there is no accessor that returns anything but the
guard itself), and the buffer cannot be handed to another thread mid-flight
(the guard is not `Send`, so `thread::spawn` rejects it), and the type that
comes back from `wait` is the original, ordinarily-usable value again.

## In-Flight Ownership Requirements

- Wrap a buffer handed to a foreign or hardware agent in a guard type that
  owns it for the duration of the operation and exposes no method that
  returns `&T`, `&mut T`, or the buffer itself while the operation is
  outstanding.
- Make the guard `!Send` (a `PhantomData<*mut ()>` marker field is enough)
  so it cannot cross a thread boundary while the transfer is in flight, even
  though the underlying buffer type may itself be `Send`.
- Give the guard exactly one way back to the buffer: a method that consumes
  `self` by value (`fn wait(self) -> T`), so the type system enforces
  "you can have the guard, or you can have the buffer, never both."
- Do not implement `Deref`/`DerefMut` from the guard to the buffer — that
  reopens the aliasing hole this type exists to close.
- Where the underlying hardware interface is itself `unsafe` (raw MMIO,
  volatile registers, a foreign driver call), keep that `unsafe` inside the
  guard's construction and `wait` implementation; the type using the guard
  should not need `unsafe` to hand a buffer off and take it back safely.

## Bad

```rust
// Handing the buffer's raw pointer to hardware and keeping the owning `Vec`
// around: nothing stops the CPU from reading it, or another thread from
// taking it, while the (simulated) transfer is still in progress.
fn start_transfer(buffer: Vec<u8>) -> Vec<u8> {
    // begin_hardware_transfer(buffer.as_ptr(), buffer.len());
    buffer // the caller can read or move this immediately
}
```

## Good

```rust
use std::marker::PhantomData;

/// Owns `T` for the duration of a transfer. Not `Send`: it cannot cross a
/// thread boundary while hardware may still be writing to the buffer.
pub struct InFlight<T> {
    buffer: T,
    _not_send: PhantomData<*mut ()>,
}

impl<T> InFlight<T> {
    /// Stands in for handing the buffer's address to a hardware engine and
    /// beginning an asynchronous transfer.
    pub fn start(buffer: T) -> Self {
        InFlight { buffer, _not_send: PhantomData }
    }

    /// The only way back to the buffer. Consuming `self` means a caller can
    /// never hold both the in-flight guard and the buffer at once.
    pub fn wait(self) -> T {
        // Stands in for polling/blocking on hardware completion before
        // releasing the buffer back to ordinary Rust ownership.
        self.buffer
    }
}

fn main() {
    let buffer = vec![0u8; 4];
    let transfer = InFlight::start(buffer);

    // `transfer.buffer` is private and `InFlight` derefs to nothing, so
    // there is no way to read the buffer while `transfer` is held — and
    // `thread::spawn(move || transfer.wait())` does not compile, because
    // `InFlight<Vec<u8>>` is `!Send`.

    let recovered = transfer.wait();
    assert_eq!(recovered, vec![0u8; 4]);
}
```

## Aliasing Cases To Test

- the buffer is recoverable, unchanged in ownership, after `wait` completes;
- no accessor on the guard exposes `&T` or `&mut T` while the guard is held
  — confirmed by the guard's field being private and no `Deref` impl
  existing, not by a runtime check;
- a build attempting `thread::spawn(move || in_flight.wait())` fails to
  compile, pinned as a compile-fail case, because the guard is `!Send`;
- constructing a new guard around the same buffer twice (starting a second
  transfer before the first completes) is unreachable, because `start`
  consumes the buffer and only `wait` gives it back.

## See Also

- [type-single-use-token](type-single-use-token.md) - the general at-most-once shape this rule specializes for exclusive hardware occupancy
- [unsafe-send-sync-manual](unsafe-send-sync-manual.md) - the auto-trait mechanics behind opting a guard out of `Send`
- [unsafe-volatile-mmio](unsafe-volatile-mmio.md) - the hardware-facing half of a real transfer this guard wraps
- [async-explicit-close](async-explicit-close.md) - a sibling pattern for a resource whose release is itself fallible or async
- [api-fallible-self-return](api-fallible-self-return.md) - handing a value back to the caller through a consuming method, the same shape `wait` uses
