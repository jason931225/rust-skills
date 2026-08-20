# async-explicit-close

> Give a resource whose release must await an explicit `close` method; leave `Drop` as a best-effort fallback

## Why It Matters

`Drop` is synchronous and there is no async version of it, so a type whose
release involves I/O — flushing a buffer, sending a protocol goodbye, returning
a lease, committing an offset — cannot do that work while being dropped. Code
that relies on `Drop` for it either blocks an executor thread inside a
destructor, or silently skips the work. Neither failure is visible at the drop
site: the data is simply never flushed, and the server sees a connection
vanish rather than close.

## Bad

```rust
impl Drop for Session {
    fn drop(&mut self) {
        // Blocks a worker inside a destructor, and panics if no runtime is
        // current — while a plain drop skips the goodbye entirely
        futures::executor::block_on(self.send_goodbye());
    }
}
```

## Good

```rust
#[derive(Debug, PartialEq)]
pub enum CloseError {
    Io,
}

pub struct Session {
    sent_goodbye: bool,
    /// Observed by the test to prove the fallback ran.
    leaked: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Session {
    pub fn new(leaked: std::sync::Arc<std::sync::atomic::AtomicBool>) -> Self {
        Self { sent_goodbye: false, leaked }
    }

    /// The supported path: takes `self`, awaits the release, and reports it.
    pub async fn close(mut self) -> Result<(), CloseError> {
        self.sent_goodbye = true;
        // await the real goodbye here
        Ok(())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if !self.sent_goodbye {
            // Best effort only: record the omission so it is visible, and do
            // no blocking work in a destructor.
            self.leaked.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

fn main() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    // Dropped without closing: the fallback records it rather than blocking.
    let leaked = Arc::new(AtomicBool::new(false));
    drop(Session::new(Arc::clone(&leaked)));
    assert!(leaked.load(Ordering::SeqCst), "an unclosed session is observable");

    // The close path is async and returns a Result the caller must handle.
    let leaked = Arc::new(AtomicBool::new(false));
    let session = Session::new(Arc::clone(&leaked));
    let closed = futures::executor::block_on(session.close());
    assert_eq!(closed, Ok(()));
    assert!(!leaked.load(Ordering::SeqCst));
}
```

## Close And Drop Responsibilities

- `close(self)` consuming the receiver stops use-after-close at compile time
  and gives the caller a `Result` to handle.
- Keep the `Drop` fallback cheap and non-blocking: log, count, or set a flag.
  Never `block_on` in a destructor — it deadlocks a current-thread runtime.
- Make the omission observable. A counter of unclosed resources turns a silent
  leak into a signal.
- Shutdown must reach the `close` calls: drain tasks with a deadline rather
  than dropping their handles.
- The same applies to explicit `flush` before dropping a buffered writer, where
  the synchronous case has the identical shape.

## See Also

- [async-cancellation-token](async-cancellation-token.md) - the drain that gives `close` a chance to run
- [perf-io-buffering](perf-io-buffering.md) - the synchronous version: drop discards the error
- [mem-drop-order](mem-drop-order.md) - what a destructor may and may not assume
- [api-fallible-self-return](api-fallible-self-return.md) - `close` consuming `self` is a fallible consuming method
