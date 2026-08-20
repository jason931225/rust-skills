# async-poll-contract

> Return from every hand-written `poll` without blocking, re-check readiness instead of trusting the wake, re-register the waker before each `Pending`, and never poll after `Ready`

## Why It Matters

A hand-written `poll` runs on an executor worker with no scheduler to rescue
it: a blocking call inside it stalls every other task on that thread, and a
`Pending` returned without a registered waker parks the task forever. Wakes
carry no payload, so a future that treats being polled as proof of readiness
decodes a buffer that is still empty, and one that stores its waker only on the
first poll misses the notification once the task moves to another worker or is
wrapped in a combinator that substitutes its own waker. None of this is a
compile error — the task hangs, completes early with garbage, or fails only
under concurrent load.

## Bad

```rust
impl Future for Download {
    type Output = Vec<u8>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Vec<u8>> {
        if !self.registered {
            // Registered once, so a later poll with a different waker is
            // never notified again.
            self.socket.register(cx.waker().clone());
            self.registered = true;
            return Poll::Pending;
        }

        // Trusts the wake: no re-check of the socket, and the read blocks the
        // executor thread when the wake was spurious.
        Poll::Ready(self.socket.read_blocking())
    }
}
```

## Good

```rust
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

/// State a producer raises from anywhere, plus the slot for the current waker.
#[derive(Default)]
pub struct Signal {
    raised: AtomicBool,
    waker: Mutex<Option<Waker>>,
}

impl Signal {
    /// Publishes readiness first, then wakes whoever is parked on it.
    pub fn raise(&self) {
        self.raised.store(true, Ordering::Release);
        if let Some(waker) = self.waker.lock().expect("signal mutex").take() {
            waker.wake();
        }
    }
}

/// Completes once the signal has been raised. Never blocks the poller.
pub struct Raised {
    signal: Arc<Signal>,
    finished: bool,
}

impl Raised {
    pub fn new(signal: Arc<Signal>) -> Self {
        Self { signal, finished: false }
    }
}

impl Future for Raised {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let this = self.get_mut();
        assert!(!this.finished, "Raised polled after returning Ready");

        // A wake is only a hint; the shared state decides readiness.
        if this.signal.raised.load(Ordering::Acquire) {
            this.finished = true;
            return Poll::Ready(());
        }

        // Register the *current* waker before parking, then load again: the
        // producer may have raised the signal between the two checks.
        *this.signal.waker.lock().expect("signal mutex") = Some(cx.waker().clone());

        if this.signal.raised.load(Ordering::Acquire) {
            this.finished = true;
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

fn main() {
    let signal = Arc::new(Signal::default());
    let mut future = Raised::new(Arc::clone(&signal));
    let mut cx = Context::from_waker(Waker::noop());

    // Not ready: Pending is returned immediately and a waker is parked.
    assert!(Pin::new(&mut future).poll(&mut cx).is_pending());
    assert!(signal.waker.lock().expect("signal mutex").is_some());

    // A spurious poll must not invent readiness, and must leave a waker behind.
    assert!(Pin::new(&mut future).poll(&mut cx).is_pending());
    assert!(signal.waker.lock().expect("signal mutex").is_some());

    signal.raise();
    assert_eq!(Pin::new(&mut future).poll(&mut cx), Poll::Ready(()));
}
```

## Poll Implementation Requirements

- Never block inside `poll`: no synchronous I/O, no `thread::sleep`, no
  contended lock held across slow work. Not-ready means return `Pending`, and
  unavoidable blocking work belongs on a blocking pool.
- Derive readiness from the observable state on every call. Being polled proves
  nothing — spurious wakes are permitted, and a wake carries no data.
- Register `cx.waker()` before every `Pending` return, not once at
  construction. The waker can differ between polls, and only the most recent
  one is guaranteed to schedule the task.
- Re-check readiness after registering, and return `Ready` if the state changed
  in between; otherwise a producer that fired during the gap wakes a waker that
  was not yet stored, and the task parks forever.
- Guarantee that every `Pending` return has a wake path: a `Pending` with no
  live registration is a hang, and it is the future's job to prove one exists.
- Treat `Ready` as terminal. Polling afterwards is unspecified, so make the
  contract violation loud, and wrap in a fused adapter when a caller genuinely
  needs to poll a completed future again.
- Keep each `poll` bounded in time; long computation inside it starves the
  worker exactly as a blocking call would.

## The Same Contract For `poll_next`

A hand-written `Stream` carries every obligation above — never block, re-check
readiness rather than trusting the wake, re-register the waker before each
`Pending` — plus a state machine `Future` does not have. `poll_next` returns
`Poll<Option<Item>>`, and the three outcomes mean different things:

- `Ready(Some(item))` yields one item and says nothing about the next. The
  consumer will poll again, so the stream must be left ready to produce or to
  park.
- `Ready(None)` is terminal, the way `Future`'s `Ready` is. Polling after it
  is unspecified; make the violation loud rather than returning items again.
- `Pending` carries the same wake obligation as a future's: something must
  wake the registered waker when another item may be available.

The common bug is a stream that returns `Ready(Some(_))` for its last item and
then, on the next poll, does the work of deciding it is finished *without*
having registered a waker for that decision — so a consumer that would have
been woken by an upstream event parks forever one item short.

```rust
use std::pin::Pin;
use std::task::{Context, Poll};

/// Yields each queued item, then ends. `finished` makes the terminal
/// contract observable instead of silently restarting.
pub struct Drain {
    items: std::collections::VecDeque<u32>,
    finished: bool,
}

impl futures::Stream for Drain {
    type Item = u32;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<u32>> {
        let this = self.get_mut();
        assert!(!this.finished, "Drain polled after returning Ready(None)");
        match this.items.pop_front() {
            Some(item) => Poll::Ready(Some(item)),
            None => {
                this.finished = true;
                Poll::Ready(None)
            }
        }
    }
}

fn main() {
    use futures::StreamExt;
    let drain = Drain {
        items: [1, 2, 3].into_iter().collect(),
        finished: false,
    };
    let collected: Vec<u32> = futures::executor::block_on(drain.collect());
    assert_eq!(collected, vec![1, 2, 3]);
}
```

Bounded *consumption* of a stream — running several items concurrently with a
ceiling — is a separate concern covered by
[async-join-parallel](async-join-parallel.md); this section is about
implementing the trait.

## See Also

- [async-cancel-safety](async-cancel-safety.md) - the future may be dropped between two polls, taking its buffered state with it
- [async-yield-cpu](async-yield-cpu.md) - bounding the work one poll performs before returning to the executor
- [conc-atomic-ordering](conc-atomic-ordering.md) - the release/acquire pairing that makes the register-then-recheck sequence correct
- [async-fn-over-future](async-fn-over-future.md) - write a manual `poll` only when `async fn` cannot express the state machine
- [async-assert-send](async-assert-send.md) - a hand-written future still owes callers the `Send` promise its fields imply
