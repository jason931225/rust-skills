# conc-condvar-predicate-loop

> Re-check a condition variable's predicate in a loop under its mutex; a wakeup is a hint, not proof

## Why It Matters

`Condvar::wait` can return without anyone having signalled it. Spurious wakeups
are permitted by the underlying platform primitives, and even a real signal
proves only that the condition held at signal time — another thread may have
consumed the work before this one reacquired the lock. Code that treats a
returned `wait` as proof proceeds on a false premise: it pops from an empty
queue, reads a half-initialised value, or deadlocks waiting for a signal that
has already been sent.

## Bad

```rust
fn take(queue: &Mutex<VecDeque<Job>>, ready: &Condvar) -> Job {
    let mut guard = queue.lock().unwrap();
    if guard.is_empty() {
        // One check, one wait: a spurious wakeup or a competing consumer
        // leaves the queue empty and this unwrap panics
        guard = ready.wait(guard).unwrap();
    }
    guard.pop_front().unwrap()
}
```

## Good

```rust
use std::collections::VecDeque;
use std::sync::{Condvar, Mutex};

pub struct Queue {
    jobs: Mutex<VecDeque<u32>>,
    ready: Condvar,
}

impl Queue {
    pub fn new() -> Self {
        Self { jobs: Mutex::new(VecDeque::new()), ready: Condvar::new() }
    }

    pub fn push(&self, job: u32) {
        self.jobs.lock().expect("not poisoned").push_back(job);
        // Signal while not holding the lock is also fine; what matters is that
        // the state was published before the notification.
        self.ready.notify_one();
    }

    pub fn take(&self) -> u32 {
        let guard = self.jobs.lock().expect("not poisoned");
        // `wait_while` is the loop: it re-acquires the lock and re-tests the
        // predicate on every wakeup, spurious or not.
        let mut guard = self
            .ready
            .wait_while(guard, |jobs| jobs.is_empty())
            .expect("not poisoned");
        guard.pop_front().expect("the predicate guarantees an element")
    }
}

fn main() {
    let queue = std::sync::Arc::new(Queue::new());
    let consumer = {
        let queue = std::sync::Arc::clone(&queue);
        std::thread::spawn(move || queue.take())
    };
    queue.push(42);
    assert_eq!(consumer.join().expect("consumer finished"), 42);
}
```

## Predicate And Notification Rules

- `wait_while` and `wait_timeout_while` are the loop written for you; a manual
  `while !predicate { guard = cv.wait(guard)? }` is equivalent.
- The predicate must read state protected by the same mutex the `Condvar` is
  paired with. A predicate over unrelated state can miss its own wakeup.
- Publish the state change before notifying, so a waiter that re-checks after
  the notification sees it.
- `notify_one` wakes an unspecified waiter; use `notify_all` when waiters are
  waiting on different predicates over the same mutex.
- A timeout does not remove the loop — `wait_timeout_while` still returns on
  spurious wakeups, and the timeout result must be checked separately.

## See Also

- [own-mutex-interior](own-mutex-interior.md) - the lock the predicate reads under
- [conc-atomic-ordering](conc-atomic-ordering.md) - publishing state before a signal
- [async-watch-latest](async-watch-latest.md) - the async equivalent for latest-value notification
- [conc-signal-handler-safety](conc-signal-handler-safety.md) - flags that ordinary code must poll
