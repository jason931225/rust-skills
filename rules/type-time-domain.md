# type-time-domain

> Measure elapsed time with `Instant`; use `SystemTime` only for timestamps that leave the process

## Why It Matters

A computer has two different clocks and they answer different questions. The
system clock reports absolute (wall clock) time and is not monotonic: NTP
steps and slews it, an operator can set it, and it carries leap-second and
time-zone baggage. A steady clock guarantees equal-length seconds that only
increase, starting from an arbitrary point near boot. Measuring a duration
with the system clock means a backwards adjustment can produce a negative or
absurd interval — which shows up as a negative latency metric, a timeout that
never fires, or a rate limiter that resets itself.

## Bad

```rust
fn handle() -> Duration {
    let start = SystemTime::now();
    do_work();
    // an NTP step during do_work() makes this panic or report nonsense
    SystemTime::now().duration_since(start).unwrap()
}
```

## Good

```rust
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub struct Completed {
    /// How long the work took: measured on the monotonic clock.
    pub elapsed: Duration,
    /// When it finished, for records other systems will read.
    pub finished_at_unix_secs: u64,
}

pub fn run<F: FnOnce()>(work: F) -> Completed {
    let started = Instant::now();
    work();
    Completed {
        elapsed: started.elapsed(),
        finished_at_unix_secs: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|since| since.as_secs())
            .unwrap_or(0),
    }
}

fn main() {
    let completed = run(|| {});
    // Monotonic: an elapsed duration always exists and never runs backwards.
    assert!(completed.elapsed < Duration::from_secs(60));
}
```

Deadlines follow the same split: compute them as `Instant::now() + budget`
inside the process, and convert to a wall-clock time only when one has to be
communicated to another system.

## Key Points

- Durations, timeouts, backoff, rate limits, caches, and latency metrics use
  `Instant`, which is monotonic.
- Values that must be stored, compared across machines, or shown to a person
  use `SystemTime` (or a date-time type over it) — those are timestamps, not
  measurements.
- Never subtract one `SystemTime` from another to time an operation, and never
  assume the difference is positive: `duration_since` returns a `Result`.
- Do not persist an `Instant` or send it anywhere. It is meaningful only within
  one process's boot epoch.
- Take the clock from an injected source in code that has to be tested, so
  tests can advance time deterministically.
- Assume timestamps arriving from other systems are skewed; do not order
  events by them without an explicit tolerance.

## See Also

- [test-mock-traits](test-mock-traits.md) - inject the clock so tests control time
- [async-bounded-dependency](async-bounded-dependency.md) - deadlines are monotonic budgets
- [obs-operational-signals](obs-operational-signals.md) - latency signals must not go negative
- [async-durable-worker](async-durable-worker.md) - backoff and retry budgets use the steady clock
