# test-drop-release-paths

> Assert that a `Drop`-based release actually happens on an early return and during a panic

## Why It Matters

A guard type's whole value is that cleanup happens on every path, including the
ones nobody wrote code for. Ordinary tests exercise the happy path, where the
release would have happened anyway, so they stay green after a refactor moves
the release from `Drop` into an explicit `close()`, or reorders fields so the
guard drops after the thing it protects. The regression is invisible until
production hits an error path or a panic, which is exactly when the cleanup
mattered.

## Bad

```rust
#[test]
fn releases_the_lease() {
    let lease = Lease::acquire();
    drop(lease);
    // Only proves the happy path: the same test passes if release moves out
    // of Drop and into an explicit method nobody calls on the error path
    assert!(released());
}
```

## Good

```rust
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Lease {
    released: Arc<AtomicUsize>,
}

impl Lease {
    pub fn acquire(released: Arc<AtomicUsize>) -> Self {
        Self { released }
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        self.released.fetch_add(1, Ordering::SeqCst);
    }
}

/// Returns early through `?` while holding the lease.
fn early_return(released: Arc<AtomicUsize>) -> Result<(), &'static str> {
    let _lease = Lease::acquire(released);
    Err("failed before the end of the scope")?;
    unreachable!()
}

fn main() {
    // Early return: the release must still happen.
    let released = Arc::new(AtomicUsize::new(0));
    assert!(early_return(Arc::clone(&released)).is_err());
    assert_eq!(released.load(Ordering::SeqCst), 1, "released on the `?` path");

    // Unwinding panic: the release must still happen.
    let released = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&released);
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _lease = Lease::acquire(counter);
        panic!("boom");
    }));
    assert!(outcome.is_err());
    assert_eq!(released.load(Ordering::SeqCst), 1, "released while unwinding");
}
```

## Assertions That Catch Regressions

- Count releases rather than asserting a boolean: a double release is as much a
  bug as none, and only a counter distinguishes them.
- Drive the two paths that ordinary tests miss — an early `?` and an unwinding
  panic via `catch_unwind` — not just an explicit `drop`.
- `panic = "abort"` skips destructors entirely; if the shipped profile aborts,
  the panic assertion documents a guarantee the binary does not have.
- Assert field drop order where a guard must outlive what it protects, since a
  field reorder is a silent regression.
- A release that must await cannot live in `Drop` at all — that is a different
  contract.

## See Also

- [test-compile-fail-guarantees](test-compile-fail-guarantees.md) - the mirror case, guarantees no runtime test can see
- [mem-drop-order](mem-drop-order.md) - the ordering these tests pin
- [async-explicit-close](async-explicit-close.md) - when the release cannot happen in `Drop`
- [err-catch-unwind-boundary](err-catch-unwind-boundary.md) - the unwinding path the second assertion drives
