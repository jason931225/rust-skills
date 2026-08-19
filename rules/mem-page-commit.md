# mem-page-commit

> Treat the first write to freshly allocated memory as the real cost and the real out-of-memory risk, not the allocation call

## Why It Matters

`Vec::with_capacity` or `vec![0; n]` asking the OS for memory does not mean
that memory exists yet. Virtual memory lets the OS overcommit: a large
allocation can succeed immediately because the kernel only promises an
address range, and the actual physical pages are not mapped — or the zero
page is mapped read-only and shared across every process that has not yet
written to it — until the first write to each page. That write is where a
copy-on-write fault happens, where physical memory is actually consumed, and
where an overcommitted system can fail with an out-of-memory kill that has
nothing to do with the original `alloc` call succeeding cleanly. A benchmark
or capacity check that only measures the allocation call is measuring the
wrong operation.

## Contract

- Do not treat a successful allocation as proof that its memory is backed;
  the first write to each page is where the real commit — and the real
  possibility of failure — happens.
- When benchmarking allocation cost, touch (write to) the allocated memory
  before timing anything downstream, or the benchmark measures address-space
  reservation, not the cost the workload will actually pay.
- Expect a large `vec![0; n]` to succeed even when the system does not have
  `n` bytes of free physical memory available; do not read that success as
  evidence the memory can actually be used.
- For latency-sensitive code, pre-fault memory that must not stall later —
  write to every page once during initialization, off the hot path, so the
  first real write does not pay for a page fault.
- Size a large, pointer-chasing working set in units of pages (commonly 4
  KiB), not just cache lines: crossing a page boundary risks a TLB miss (a
  second memory access to look up the translation) on top of any cache miss,
  and that cost multiplies under nested virtualization or containers.
- Re-measure on the actual deployment target — overcommit policy, page size,
  and TLB behavior are OS- and hardware-specific, not portable constants.

## Bad

```rust
use std::time::Instant;

// Measures only how long the allocator takes to reserve address space. The
// pages are not backed by physical memory yet, so this benchmark says
// nothing about the cost the workload pays once it actually writes to them.
fn benchmark_allocation(size: usize) -> std::time::Duration {
    let start = Instant::now();
    let _buffer: Vec<u8> = Vec::with_capacity(size);
    start.elapsed()
}
```

## Good

```rust
const PAGE: usize = 4096;

/// Touches every page in the allocation instead of trusting the allocation
/// call alone. Returns how many pages were actually written to, so a
/// benchmark wrapping this can attribute time to the touch, not just the
/// reservation.
fn allocate_and_first_touch(size: usize) -> (Vec<u8>, usize) {
    let mut buffer = vec![0u8; size];
    let mut touched = 0;
    let mut offset = 0;
    while offset < buffer.len() {
        buffer[offset] = 1; // the write that actually commits this page
        touched += 1;
        offset += PAGE;
    }
    (buffer, touched)
}

fn main() {
    let size = 16 * PAGE;
    let (buffer, touched) = allocate_and_first_touch(size);

    assert_eq!(touched, 16, "every page in the allocation was written to, not just reserved");
    // Every page's first byte was actually committed; a version that only
    // called `vec![0; size]` would still pass this on most systems, but
    // would not have paid (or measured) the per-page fault this function
    // forces up front instead of on first real use.
    for page in 0..touched {
        assert_eq!(buffer[page * PAGE], 1);
    }
}
```

## See Also

- [opt-cache-friendly](opt-cache-friendly.md) - the cache-line-level version of this locality argument; this rule is the page/TLB level above it
- [mem-with-capacity](mem-with-capacity.md) - reserving capacity is the allocation half; this rule covers what happens after
- [perf-profile-first](perf-profile-first.md) - measuring the wrong operation (allocation instead of first touch) is exactly the trap profiling catches
- [mem-arena-allocator](mem-arena-allocator.md) - batch allocation strategies that make first-touch cost a one-time, predictable event
- [proj-reproducible-runtime](proj-reproducible-runtime.md) - overcommit policy and page size are properties of the runtime environment, not the code
