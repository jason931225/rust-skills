# perf-global-allocator

> Pick the process global allocator on purpose in application crates; leave libraries on the system default

## Why It Matters

Library crates must not install a `#[global_allocator]`: the application owns the process heap, and two crates fighting over it is a link error. The system allocator is a fine default; replacing it is a measured application decision for allocation-heavy workloads. `mimalloc`, `jemalloc`, `snmalloc`, and the system heap are all candidates once representative benchmarks justify one. Put the `static` in `main.rs`, never in a published `lib.rs`.

## Bad

```rust
// inside a library crate
use std::alloc::{GlobalAlloc, Layout, System};

struct LibHeap;

unsafe impl GlobalAlloc for LibHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: this implementation forwards the allocator contract unchanged.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: callers must pass the pointer and layout returned by this allocator.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static LIB_HEAP: LibHeap = LibHeap;
```

## Good

```rust
// src/main.rs of an application
use std::alloc::{GlobalAlloc, Layout, System};

struct AppHeap;

unsafe impl GlobalAlloc for AppHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // Production apps often swap `System` for `mimalloc::MiMalloc` here
        // after a benchmark; the choice stays in the binary, not a library.
        // SAFETY: this implementation forwards the allocator contract unchanged.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: callers must pass the pointer and layout returned by this allocator.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static HEAP: AppHeap = AppHeap;

fn main() {
    let _v = vec![1, 2, 3];
}
```

## Freestanding Targets Must Supply One, And Initialise It First

Everything above is about *choosing* an allocator when a default already
exists. On a freestanding target there is no default: linking `alloc` without
a `#[global_allocator]` is a hard error, and the allocator you supply owns a
heap region you have to hand it explicitly. That introduces an ordering
hazard a hosted program never has — the allocator exists as a `static` from
the moment the program starts, but its heap is not usable until `init` has
run, and an allocation in between is undefined behaviour rather than a clean
failure.

- Initialise the heap as the first thing in `main`/`_start`, before anything
  that could allocate. Reaching the allocator before `init` typically faults
  or silently hands back a pointer into unmapped memory.
- Give the heap a region that provably does not overlap `.data`, `.bss`, or
  the stack. Those bounds come from the linker script, not from a guess; an
  overlap corrupts statics or the stack under load rather than at startup.
- Initialise exactly once. A second `init` re-arms the allocator over memory
  already handed out, which turns every live allocation into a dangling one.
- Watch for anything that allocates before your init runs — a `lazy_static`,
  a pre-main constructor, or a HAL setup routine that builds a `Vec`. The
  ordering bug is in the caller, so it moves when unrelated code is added.
- Decide what allocation failure does. There is nowhere to unwind to, so the
  handler runs the same policy question as
  [err-panic-handler-policy](err-panic-handler-policy.md): report if a channel
  exists, then halt or reset deliberately.
- Prefer not allocating at all where the workload allows it. A fixed-capacity
  collection sidesteps this entire contract
  ([mem-arrayvec](mem-arrayvec.md)).

```rust
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Models the state machine a freestanding allocator has to enforce. The
/// real type also implements `GlobalAlloc` and is installed with
/// `#[global_allocator]`; the part worth pinning is that using it before
/// `init` is a distinguishable error rather than a silent bad pointer.
pub struct FreestandingHeap {
    ready: AtomicBool,
    remaining: AtomicUsize,
}

#[derive(Debug, PartialEq)]
pub enum HeapError {
    NotInitialised,
    AlreadyInitialised,
    Exhausted,
}

impl FreestandingHeap {
    pub const fn new() -> Self {
        Self { ready: AtomicBool::new(false), remaining: AtomicUsize::new(0) }
    }

    /// Hands the allocator its region. Returns an error on a second call
    /// rather than re-arming over memory already given out.
    pub fn init(&self, size: usize) -> Result<(), HeapError> {
        if self.ready.swap(true, Ordering::SeqCst) {
            return Err(HeapError::AlreadyInitialised);
        }
        self.remaining.store(size, Ordering::SeqCst);
        Ok(())
    }

    pub fn allocate(&self, bytes: usize) -> Result<usize, HeapError> {
        if !self.ready.load(Ordering::SeqCst) {
            return Err(HeapError::NotInitialised);
        }
        let left = self.remaining.load(Ordering::SeqCst);
        if bytes > left {
            return Err(HeapError::Exhausted);
        }
        self.remaining.store(left - bytes, Ordering::SeqCst);
        Ok(bytes)
    }
}

fn main() {
    let heap = FreestandingHeap::new();

    // The hazard this section exists for: allocating before init.
    assert_eq!(heap.allocate(16), Err(HeapError::NotInitialised));

    assert_eq!(heap.init(64), Ok(()));
    // A second init would re-arm over live allocations.
    assert_eq!(heap.init(64), Err(HeapError::AlreadyInitialised));

    assert_eq!(heap.allocate(16), Ok(16));
    assert_eq!(heap.allocate(1024), Err(HeapError::Exhausted));
}
```

## See Also

- [proj-lib-main-split](proj-lib-main-split.md) - the allocator static lives next to `main`, not in `lib.rs`
- [perf-profile-first](perf-profile-first.md) - change the heap only after a profile says allocation is the cost
- [mem-arena-allocator](mem-arena-allocator.md) - scoped arenas are a library-safe alternative to a process-global swap
