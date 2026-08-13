# async-durable-worker

> Claim durable work atomically, bound retries with backoff and jitter, and make worker shutdown explicit

## Why It Matters

An in-memory spawned task disappears on process crash. A loop that polls an
empty database without delay creates load while doing nothing. A worker that
deletes a task before completing the side effect loses work; one that never
records attempts retries poison tasks forever. Store work durably and model the
claim, lease, completion, retry, and dead-letter transitions.

## Bad

- Spawn detached in-memory work after committing business state.
- Delete a task before its side effect is durably complete.
- Hot-poll an empty queue or retry poison work forever.

## Good

- Enqueue business state and the work record in one local transaction (outbox
  pattern).
- Claim a bounded batch atomically using a lease/visibility timeout or database
  locking that allows other workers to skip claimed rows. Increment and persist
  the attempt in that same transaction so a process kill still consumes an
  attempt.
- Make task processing idempotent because a crash can occur after the side
  effect but before acknowledgement. Use a fencing token when the external
  side effect can reject a worker whose lease has expired.
- Acknowledge/delete only after the success condition is durable.
- Classify permanent versus transient failures. Retry transient failures with
  capped exponential backoff and jitter, honoring an attempt and age budget.
- Move exhausted or permanent failures to a queryable dead-letter state with
  operator context. Store references and stable error classes, not credentials,
  tokens, personal data, or a verbatim payload; define retention and purge.
- Block, notify, or sleep when the queue is empty; never hot-poll.
- Bound per-task execution and in-flight tasks per worker.
- Observe queue depth, oldest age, attempts, success/failure rate, dead-letter
  depth and arrival rate, worker heartbeat, and originating correlation ID.
- On shutdown, stop claiming, finish or release leases within a deadline, and
  join the worker task. When the deadline expires, leave the durable lease to
  expire or fence the worker; never acknowledge unfinished work.

## State Model

```rust
use std::time::SystemTime;

pub enum WorkState {
    Pending { attempt: u32 },
    Leased {
        attempt: u32,
        owner: u64,
        fencing_token: u64,
        lease_expires_at: SystemTime,
    },
    Completed,
    DeadLetter {
        attempt: u32,
        error_class: &'static str,
        failed_at: SystemTime,
    },
}

pub fn retry_ceiling(base_ms: u64, attempt: u32, max_delay_ms: u64) -> u64 {
    base_ms
        .saturating_mul(1_u64 << attempt.min(16))
        .min(max_delay_ms)
}

pub fn retry_delay(
    base_ms: u64,
    attempt: u32,
    max_delay_ms: u64,
    sampled_delay_ms: u64,
) -> u64 {
    sampled_delay_ms.min(retry_ceiling(base_ms, attempt, max_delay_ms))
}

fn main() {
    // The caller samples a fresh value uniformly from 0..=retry_ceiling(...)
    // for each attempt. Passing the sample keeps this example deterministic.
    assert_eq!(retry_ceiling(100, 3, 5_000), 800);
    assert_eq!(retry_delay(100, 3, 5_000, 431), 431);
}
```

Sampling a fresh delay from zero through the capped exponential ceiling is
full jitter. A constant added by every worker is not jitter and does not
decorrelate retries.

## Failure Tests

- crash before claim, after claim, after side effect, and before acknowledge;
- lease expiry makes abandoned work visible again;
- a worker killed without unwinding consumes exactly one attempt;
- two workers never own the same lease record simultaneously; idempotency or a
  fencing token, not the lease alone, prevents duplicate external effects;
- empty queues do not generate unbounded queries;
- poison work reaches dead-letter after the exact attempt budget;
- task timeout, cancellation, and shutdown-deadline exhaustion leave work in
  the documented retry or completion state;
- dead-letter retention purges expired operator context.

## See Also

- [async-bounded-channel](async-bounded-channel.md) - bound in-process handoff
- [async-cancellation-token](async-cancellation-token.md) - coordinate shutdown
- [async-joinset-structured](async-joinset-structured.md) - retain and join worker tasks
- [api-idempotency-key](api-idempotency-key.md) - make repeated side-effect attempts safe
- [conc-db-transaction-boundary](conc-db-transaction-boundary.md) - atomically enqueue outbox work
