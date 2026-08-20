# conc-pattern-choice

> Share memory only when concurrent updates do not commute; otherwise give the state one owner and send it messages

## Why It Matters

The concurrency pattern is usually not chosen at all — `Arc<Mutex<T>>` is the
nearest primitive, so it becomes the design, and every later problem is a lock
problem. There is a test that decides it instead: if one thread updates state
`s` with `f` and another with `g`, and `f(g(s))` equals `g(f(s))`, the threads
never needed to see each other's partial results, and a lock buys nothing but
serialization. Choosing wrongly is not merely slower — the aggregate cost of
locks, channels, and atomics can make a parallel program lose to its
single-threaded version on any number of cores, which is why the decision has
to be measured before and after rather than assumed.

## The Commutativity Test

Ask what happens if the two updates land in the other order.

- **They commute** — counting, summing, union, max, appending to a log whose
  order is not part of the contract. Give each worker its own accumulator and
  combine at the end, or funnel results to one owner. No shared mutable state
  exists, so no lock can be misused.
- **They do not commute** — "withdraw if funds allow" against "apply
  interest", or any read-decide-write where the decision depends on what the
  other thread is about to change. These need mutual exclusion. A single
  owning thread supplies it without a lock; a mutex supplies it with one.
- **You cannot tell** — that is itself the answer. State whose update order
  matters in ways nobody has written down is state that should have one owner
  until somebody does.

## Bad

```rust
use std::sync::{Arc, Mutex};
use std::thread;

// Addition commutes, so no worker ever needs another's partial sum. The lock
// does no ordering work here; it only serializes threads that had no reason
// to wait for each other, once per element.
fn total(chunks: Vec<Vec<u64>>) -> u64 {
    let total = Arc::new(Mutex::new(0u64));
    let mut handles = Vec::new();
    for chunk in chunks {
        let total = Arc::clone(&total);
        handles.push(thread::spawn(move || {
            for value in chunk {
                *total.lock().expect("total mutex") += value;
            }
        }));
    }
    for handle in handles {
        handle.join().expect("worker panicked");
    }
    let total = total.lock().expect("total mutex");
    *total
}

fn main() {
    assert_eq!(total(vec![vec![1, 2, 3], vec![4, 5], vec![6]]), 21);
}
```

## Good

```rust
use std::sync::mpsc;
use std::thread;

// Each worker owns its accumulator and reports once. There is no shared
// mutable state to lock, so the double-acquisition and held-guard failures
// cannot occur — not because they are avoided, but because nothing is shared.
fn total(chunks: Vec<Vec<u64>>) -> u64 {
    let (tx, rx) = mpsc::channel();
    for chunk in chunks {
        let tx = tx.clone();
        thread::spawn(move || {
            let subtotal: u64 = chunk.iter().sum();
            tx.send(subtotal).expect("the receiver outlives the workers");
        });
    }
    // The original sender must go, or the receive loop never ends.
    drop(tx);
    rx.iter().sum()
}

fn main() {
    assert_eq!(total(vec![vec![1, 2, 3], vec![4, 5], vec![6]]), 21);
    assert_eq!(total(Vec::new()), 0, "no workers is an empty sum, not a hang");
}
```

## When Shared Memory Earns Its Lock

A single owner also provides mutual exclusion, so "these updates do not
commute" is an argument for exclusion, not specifically for a mutex. Shared
memory wins when funnelling every operation through one owner is itself the
bottleneck:

- **Readers dominate.** A reader/writer lock lets many threads read at once;
  one owner would serialize reads that could have run in parallel. Measure it
  rather than assuming — a plain mutex may still win at low core counts.
- **The structure is large and the work is disjoint.** Threads operating on
  separate subranges of one `Vec` are sharing an address, not contending for
  state, and copying it per worker would cost more than the coordination.
- **The per-message cost dominates the work.** An owner that does a
  nanosecond of work per message spends all its time on the queue.

Each of these is a measurement, not a preference. Say which one applies.

## Matching The Pattern To The Work

- **Worker pool** — every thread runs the *same* code on *different* data:
  `rayon`'s parallel map, an async runtime polling tasks. The diagnostic is
  that once you start distinguishing between the threads in the pool, giving
  them different roles or different state, the pool is the wrong shape and you
  want owners instead. Reuse a pool that already handles work stealing rather
  than building one; a naive queue becomes the bottleneck it was meant to
  remove.
- **Single owner** — one thread or task owns one resource (a connection, a
  file, a metrics table), and everything else interacts by message. Exclusive
  ownership means no synchronization inside the owner at all, and the owner's
  invariant lives in one place instead of at every call site that could have
  taken a guard.
- **Shared memory** — the cases above, entered deliberately, with the data
  structure chosen for the access pattern rather than defaulted to `Mutex`.

## See Also

- [conc-lock-reentry](conc-lock-reentry.md) - the failure that shared memory invites, and how to bound it
- [conc-thread-channel](conc-thread-channel.md) - bounding the queue and using disconnection as shutdown
- [conc-rayon-par-iter](conc-rayon-par-iter.md) - the worker-pool pattern for data parallelism
- [own-mutex-interior](own-mutex-interior.md) - using `Mutex<T>` once shared memory is the decision
