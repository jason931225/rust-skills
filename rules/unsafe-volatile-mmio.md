# unsafe-volatile-mmio

> Reach memory-mapped hardware through `read_volatile`/`write_volatile`, never through an ordinary reference

## Why It Matters

A store to a device register has an effect the compiler cannot see. To an
optimiser, writing a value that is never read back is dead code, two writes to
the same address collapse into one, and a loop polling a status word can be
hoisted out entirely because nothing in the program changes it. All three
transformations are legal on ordinary memory and all three break a driver.
Volatile access tells the compiler each operation must happen, exactly once,
in the order written.

## Bad

```rust
unsafe fn clear_screen(framebuffer: *mut u8) {
    for offset in 0..2000 {
        // Ordinary writes to memory nothing reads: the optimiser may drop the
        // whole loop
        *framebuffer.add(offset * 2) = b' ';
    }
}
```

## Good

```rust
/// Writes one cell of a text-mode framebuffer.
///
/// # Safety
///
/// `framebuffer` must point to a mapping of at least `cells * 2` bytes that is
/// valid for writes for the duration of the call, and no other code may write
/// the same region concurrently.
unsafe fn write_cell(framebuffer: *mut u8, cell: usize, byte: u8, colour: u8) {
    // SAFETY: the caller guarantees the mapping covers `cell`, and volatile
    // writes are required because the effect is on the device, not on memory
    // the program reads back.
    unsafe {
        framebuffer.add(cell * 2).write_volatile(byte);
        framebuffer.add(cell * 2 + 1).write_volatile(colour);
    }
}

fn main() {
    // Stand-in for a mapping: the contract is identical, only the address
    // differs on real hardware.
    let mut buffer = vec![0u8; 8];
    let pointer = buffer.as_mut_ptr();

    // SAFETY: the buffer above is exactly four cells and outlives the call.
    unsafe {
        write_cell(pointer, 0, b'O', 0x0f);
        write_cell(pointer, 1, b'K', 0x0f);
    }

    assert_eq!(&buffer[..4], &[b'O', 0x0f, b'K', 0x0f]);
}
```

## Key Points

- Volatile controls elision and reordering *by the compiler*. It is not atomic,
  not a memory barrier, and not thread synchronisation — use atomics or fences
  where another agent races you.
- Read a register once into a local when the value must be stable; a second
  volatile read is a second bus transaction and may return something else.
- Match the access width the device expects; splitting a 32-bit register into
  byte writes is a different transaction.
- Wrap the mapping in a type that owns it, so the raw pointer does not travel
  through ordinary code.
- The same reasoning applies to memory shared with a device or another process
  outside Rust's model.

## See Also

- [unsafe-safety-comment](unsafe-safety-comment.md) - the proof each block carries
- [unsafe-justify-use](unsafe-justify-use.md) - platform access is one of the legitimate reasons
- [conc-atomic-ordering](conc-atomic-ordering.md) - what volatile does not give you
- [ffi-status-to-result](ffi-status-to-result.md) - the other half of a platform boundary
