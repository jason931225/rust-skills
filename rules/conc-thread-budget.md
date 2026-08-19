# conc-thread-budget

> Bound OS-thread count by what the workload actually does — CPU-bound work collapses past the physical core count, sleeping threads do not

## Why It Matters

"More threads means more concurrency" holds only for a workload that mostly
waits — a thread parked on I/O costs the scheduler almost nothing, and a few
hundred sleeping threads are cheap. A CPU-bound workload is a different
system entirely: once thread count exceeds the number of physical cores,
every additional thread only adds context-switch overhead and cache
eviction, and throughput falls rather than plateaus. Conflating the two knees
— scheduler variance for idle threads versus core oversubscription for busy
ones — produces a thread pool sized by folklore instead of by what the
threads in it actually do. `thread::sleep` compounds the problem for
short or jitter-sensitive pauses: it is a request to the OS scheduler, not a
deadline, and tick coalescing can make "sleep 20 ms" run measurably longer.

## Contract

- Size a CPU-bound worker pool to the number of physical cores available to
  the process, not to a fixed constant or the number of pending jobs; measure
  throughput against core count to find the actual knee.
- Size a pool of mostly-blocked threads (waiting on I/O, a channel, a lock)
  by memory and scheduler overhead instead — hundreds of parked threads stay
  cheap as long as they are not all runnable at once.
- Do not use `thread::sleep` for a short or jitter-sensitive deadline without
  accounting for OS tick granularity; sleep the bulk of the interval and spin
  on `Instant::now()` for the remainder when the pause must land close to on
  time.
- Distinguish `std::thread::yield_now()` from `std::hint::spin_loop()`:
  `yield_now` unschedules the thread with no bound on when the OS resumes it;
  `spin_loop` is a CPU-level pause hint with no scheduling effect at all (and
  is a no-op on some architectures). Using one where the other is needed
  either burns a core or injects unbounded delay.
- Re-measure the thread count on the deployment target, not the development
  machine; core count, hyperthreading, and container CPU limits all change
  where the CPU-bound knee sits.

## Bad

```rust
// A CPU-bound pool sized to the job queue depth rather than to available
// cores. Past physical-core count, every additional thread adds context
// switches and cache eviction instead of throughput.
fn spawn_pool(job_count: usize) -> usize {
    job_count // "more jobs pending, so spawn more threads"
}
```

## Good

```rust
use std::time::{Duration, Instant};

/// A CPU-bound pool is sized to physical cores, never to job count.
fn cpu_bound_pool_size(available_cores: usize) -> usize {
    available_cores.max(1)
}

/// Sleeps most of `target`, then spins on the clock for the remainder, so
/// the wake time lands close to `target` instead of drifting with whatever
/// the OS scheduler's tick granularity happens to allow.
fn precise_pause(target: Duration) {
    let start = Instant::now();
    let sleep_portion = target.saturating_sub(Duration::from_millis(2));
    if !sleep_portion.is_zero() {
        std::thread::sleep(sleep_portion);
    }
    while start.elapsed() < target {
        std::hint::spin_loop();
    }
}

fn main() {
    assert_eq!(cpu_bound_pool_size(8), 8);
    assert_eq!(cpu_bound_pool_size(0), 1, "never size a pool to zero threads");

    let target = Duration::from_millis(5);
    let start = Instant::now();
    precise_pause(target);
    assert!(start.elapsed() >= target, "the pause never returns early");
}
```

## See Also

- [conc-rayon-par-iter](conc-rayon-par-iter.md) - rayon already sizes its pool to cores; this rule is for hand-rolled thread pools
- [async-yield-cpu](async-yield-cpu.md) - the equivalent budget for cooperative tasks on an executor, not OS threads
- [async-spawn-blocking](async-spawn-blocking.md) - moving blocking work off a pool that is sized for CPU-bound throughput
- [type-time-domain](type-time-domain.md) - `Instant` is the clock this rule's spin phase measures against
- [perf-profile-first](perf-profile-first.md) - find the actual knee by measurement, not by assuming one thread count fits every workload
