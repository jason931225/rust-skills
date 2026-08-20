# conc-thread-channel

> Bound a thread-to-thread channel, and treat sender disconnection as the shutdown signal rather than a separate protocol

## Why It Matters

A threaded program that shares state through `Arc<Mutex<T>>` makes every
participant contend for one lock and leaves the invariant spread across every
call site that takes the guard. Moving the state into one owning thread and
sending it messages replaces contention with a queue — but only if the queue
is bounded and the shutdown path is real. `std::sync::mpsc::channel()` is
unbounded: a producer that outruns its consumer grows the queue until the
process dies, and it does so silently because nothing ever blocks. And the
shutdown signal is already in the type — when every `Sender` is dropped,
`recv()` returns `Err`, which is the cleanest termination condition a worker
loop can have and is routinely reimplemented as a redundant `Shutdown`
message instead.

## Choosing And Wiring The Channel

- Use `sync_channel(n)` with a deliberate capacity for thread-to-thread work.
  `channel()` is unbounded — reach for it only when the producer is provably
  bounded by something else, and say what that is.
- `sync_channel(0)` is a rendezvous: the send blocks until a receiver takes
  the value, so the two threads are lockstep. That is a synchronisation choice,
  not a zero-sized buffer, and it deadlocks if the receiver can ever be waiting
  on the sender.
- End the consumer by dropping every `Sender`. `for msg in rx` and
  `while let Ok(msg) = rx.recv()` both terminate on disconnection, so a
  separate shutdown message is redundant — and a shutdown message that races
  the queue can be processed before the work ahead of it.
- Watch for the sender you forgot to drop: a `Sender` still held by the
  spawning scope, or cloned into a structure that outlives the producers,
  keeps the consumer blocked in `recv()` forever. This is the usual cause of
  a join that never returns.
- `std::sync::mpsc` is multi-producer, **single**-consumer. For several
  consumers pulling from one queue, or for selecting across several channels,
  use a maintained MPMC channel — `std` has no `select`.
- Keep this separate from an async runtime: a blocking `recv()` on an executor
  thread stalls every task on it ([async-mpsc-queue](async-mpsc-queue.md)
  covers the async side, and its own Bad example is precisely a sync channel
  used inside async).

## Bad

```rust
use std::sync::mpsc;
use std::thread;

fn run() {
    // Unbounded: if the producer outruns the consumer the queue grows until
    // the process is killed, and nothing blocks to signal it.
    let (tx, rx) = mpsc::channel::<u64>();

    for id in 0..4 {
        let tx = tx.clone();
        thread::spawn(move || {
            for n in 0..100_000 {
                tx.send(id * n).expect("receiver alive");
            }
        });
    }

    // `tx` is still in scope here, so the channel never disconnects and this
    // loop never ends — the join below waits forever.
    for value in rx {
        let _ = value;
    }
}
```

## Good

```rust
use std::sync::mpsc;
use std::thread;

/// Owns the state outright; nothing else can reach it, so there is no lock.
struct Totals {
    seen: u64,
    sum: u64,
}

pub fn accumulate(producers: usize, per_producer: u64) -> Totals {
    // Bounded: a producer that outruns the consumer blocks here rather than
    // growing the queue, which is the backpressure signal.
    let (tx, rx) = mpsc::sync_channel::<u64>(64);

    let mut handles = Vec::new();
    for id in 0..producers {
        let tx = tx.clone();
        handles.push(thread::spawn(move || {
            for n in 0..per_producer {
                tx.send(id as u64 + n).expect("consumer is alive");
            }
        }));
    }
    // The original sender must go, or the channel never disconnects and the
    // consumer loop below never terminates.
    drop(tx);

    let mut totals = Totals { seen: 0, sum: 0 };
    // Ends when every clone of the sender has been dropped — no shutdown
    // message, and no chance of a shutdown racing ahead of queued work.
    for value in rx {
        totals.seen += 1;
        totals.sum += value;
    }

    for handle in handles {
        handle.join().expect("producer did not panic");
    }
    totals
}

fn main() {
    let totals = accumulate(4, 1_000);
    assert_eq!(totals.seen, 4_000, "every message was received before shutdown");
}
```

## Cases To Pin In Tests

- every message sent is received before the loop ends — disconnection must
  not drop queued work;
- dropping the original `Sender` is what terminates the consumer; keeping it
  alive is the hang, and a test that never joins will not catch it;
- a bounded channel actually blocks the producer once full, rather than
  growing — assert on the observed queue depth or on producer progress;
- the state lives in exactly one thread, so no test needs a lock to read it
  consistently.

## Waiting On Work, A Tick, And A Deadline In One Loop

This rule notes that `std` has no `select` and moves on. That leaves the common
shape unanswered: a worker that must serve a work queue, do something
periodically, and stop at a deadline.

The alternative people reach for is sequencing non-blocking receives —
`if let Ok(job) = work.try_recv() { .. } else if let Ok(_) = tick.try_recv() { .. }`
— and it starves everything after the first arm. Over 2000 iterations with both
channels always ready, the sequenced form gave the first channel 2000 turns and
the second 0. A real `select` over the same channels gave 1007 and 993, because
its arm order is randomised for exactly this reason.

So take the MPMC channel this rule already points at and use its `select!`:
one loop, one arm per source, and a receive on a timer channel instead of a
deadline flag threaded through the body.

```text
one select! loop over work + tick + deadline:
  jobs=7 ticks=5 ended: deadline elapsed
```

Two things stay true from the rest of this rule. Disconnection is still the
shutdown path — a `recv` arm that reports the channel closed ends the loop, so
there is no sentinel message and no separate stop flag. And the deadline is a
channel like any other, which is what keeps it out of the work-handling code:
nothing has to check elapsed time between jobs, because waiting on the deadline
is one of the things the loop is already waiting on.

Where the set of sources is known only at runtime, the same crates offer a
dynamic form that takes a built list of operations rather than a fixed set of
arms.

## See Also

- [async-mpsc-queue](async-mpsc-queue.md) - the async counterpart, and why a sync channel inside async is a different mistake
- [async-bounded-channel](async-bounded-channel.md) - the same backpressure argument on the async side
- [own-mutex-interior](own-mutex-interior.md) - the shared-state alternative this rule trades a queue against
- [conc-thread-budget](conc-thread-budget.md) - sizing the producer and consumer threads this channel connects
- [conc-scoped-threads](conc-scoped-threads.md) - borrowing stack data across the threads at both ends
