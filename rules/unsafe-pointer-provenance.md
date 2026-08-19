# unsafe-pointer-provenance

> Keep every `offset`/`add`/`sub` result inside the allocation it started from, even when the result is never dereferenced

## Why It Matters

A pointer in Rust carries more than an address: it carries *provenance*, a
record of which allocation it is allowed to access. `ptr.add(n)` or
`ptr.offset(n)` that lands outside the bounds of the allocation `ptr` points
into is undefined behavior the moment the arithmetic happens — not later, when
something dereferences the result, and not only if the resulting address
happens to overlap a different live object. "I never read through it" is not a
defense, because the UB is in forming the invalid pointer, not in using it.
This is the opposite of what pointer arithmetic looks like in C, where
addresses are just integers with no notion of which object they came from, and
it is why two pointers with the same numeric address are not interchangeable
if they carry different provenance.

## Contract

- Keep every intermediate pointer produced by `offset`/`add`/`sub`/`wrapping_*`
  within the bounds of the single allocation the base pointer points into — one
  past the end is the only exception `offset`/`add` permit, and only as a
  target for comparison, never for a read.
- Do not walk past an allocation's end "temporarily" to compute an address and
  step back; form the out-of-bounds pointer at all and the arithmetic is UB,
  independent of whether anything is ever read through it.
- Use `wrapping_add`/`wrapping_sub` only when you accept losing the resulting
  pointer's provenance for a later dereference; they define the arithmetic but
  do not exempt the pointer from bounds rules if you dereference it.
- Do not reconstruct a pointer from an integer you saved earlier and expect it
  to carry the provenance of the object it once pointed into; use
  `with_addr`/`expose_provenance`/`from_exposed_addr` (or keep the pointer
  itself, not its address) when the round trip is unavoidable.
- Two pointers with the same address but different provenance are not
  interchangeable — a pointer derived from allocation A cannot be used to
  access allocation B even if their addresses happen to coincide.
- Run pointer-manipulating code under Miri; provenance violations are UB with
  no reliable crash, so a passing test proves nothing about this contract.

## Bad

```rust
// Computing an address 1 element past a 4-element array by adding one more
// than the array holds. The intent is "walk to where the next allocation
// might start," but forming that pointer is already UB — independent of
// whether `end` is ever dereferenced.
fn past_the_end(array: &[u32; 4]) -> *const u32 {
    let base = array.as_ptr();
    unsafe { base.add(5) } // one past `add(4)`, the only pointer this permits
}
```

## Good

```rust
/// Every intermediate pointer stays within `[data, data + len]`; `add(len)`
/// itself is allowed as a one-past-the-end sentinel, `add(len + 1)` is not.
fn sum_via_pointers(data: &[u32]) -> u32 {
    let mut total = 0u32;
    let mut cursor = data.as_ptr();
    // SAFETY: `end` is `data.as_ptr().add(data.len())`, the one permitted
    // one-past-the-end pointer. Every `cursor` value in the loop satisfies
    // `data.as_ptr() <= cursor < end`, so `add(1)` never leaves the
    // allocation, and `cursor` is only read while `cursor < end`.
    let end = unsafe { data.as_ptr().add(data.len()) };
    while cursor < end {
        total = total.wrapping_add(unsafe { *cursor });
        cursor = unsafe { cursor.add(1) };
    }
    total
}

fn main() {
    let data = [1u32, 2, 3, 4];
    assert_eq!(sum_via_pointers(&data), 10);

    let empty: [u32; 0] = [];
    // `add(0)` on an empty slice's pointer is exactly the one-past-the-end
    // case the contract allows, and the loop never dereferences it.
    assert_eq!(sum_via_pointers(&empty), 0);
}
```

## Failure Tests

- summing an empty slice never dereferences a pointer, exercising the
  zero-length one-past-the-end case;
- summing a slice of length one produces one `add`, landing exactly at the
  permitted one-past-the-end pointer and stopping the loop before it derefs;
- `cursor < end` is checked before every dereference, so the loop cannot read
  the one-past-the-end sentinel itself;
- run under Miri: a version that computes `add(len + 1)` even without
  dereferencing it is flagged, while the version above is clean;
- a pointer saved as a `usize` and reconstructed later is not substituted for
  a live pointer in the hot path — the function never performs that round trip.

## See Also

- [unsafe-byte-slice-cast](unsafe-byte-slice-cast.md) - the bounds and alignment obligations for the value the pointer eventually reads
- [unsafe-maybeuninit](unsafe-maybeuninit.md) - a pointer into uninitialized memory has the same provenance rules plus a validity gap
- [unsafe-safety-comment](unsafe-safety-comment.md) - the local proof that every arithmetic step stays in bounds belongs in the comment, not in a passing test
- [unsafe-miri-ci](unsafe-miri-ci.md) - the only tool that reliably catches a provenance violation that a debug build would not
- [opt-bounds-check](opt-bounds-check.md) - the safe-iterator alternative that never risks forming an out-of-bounds pointer
