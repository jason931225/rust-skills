# async-bounded-dependency

> Bound dependency admission and calls with explicit deadlines and observable failures

## Why It Matters

Every database pool, RPC client, and external service has finite capacity.
Unbounded acquisition or operation waits turn saturation and misconfiguration
into growing request queues that exhaust memory and hide the original failure.
Set separate connect, admission, and operation deadlines so callers and
operators can distinguish overload from dependency errors.

## Bad

```rust
async fn load_user(pool: &Pool, id: u64) -> Result<User, Error> {
    let connection = pool.acquire().await?;
    connection.load_user(id).await
}

struct Pool;
struct Connection;
struct User;
struct Error;

impl Pool {
    async fn acquire(&self) -> Result<Connection, Error> {
        Ok(Connection)
    }
}

impl Connection {
    async fn load_user(&self, _id: u64) -> Result<User, Error> {
        Ok(User)
    }
}
```

Both waits can occupy the request forever when the pool or dependency stalls.

## Good

```rust
use std::time::Duration;
use tokio::time::{timeout, error::Elapsed};

#[derive(Debug)]
enum LoadError {
    PoolClosed,
    AcquireTimeout,
    QueryTimeout,
    Query,
}

async fn load_user(pool: &Pool, id: u64) -> Result<User, LoadError> {
    let connection = timeout(Duration::from_millis(250), pool.acquire())
        .await
        .map_err(|_: Elapsed| LoadError::AcquireTimeout)?
        .map_err(|_| LoadError::PoolClosed)?;

    timeout(Duration::from_secs(2), connection.load_user(id))
        .await
        .map_err(|_: Elapsed| LoadError::QueryTimeout)?
        .map_err(|_| LoadError::Query)
}

struct Pool;
struct Connection;
struct User;
struct Error;

impl Pool {
    async fn acquire(&self) -> Result<Connection, Error> {
        Ok(Connection)
    }
}

impl Connection {
    async fn load_user(&self, _id: u64) -> Result<User, Error> {
        Ok(User)
    }
}
```

## Operational Contract

- Configure finite, non-zero connect, acquire, and operation deadlines from
  typed settings; the caller's total deadline remains the upper bound.
- Bound pool size and request admission together. A larger pool can move
  overload into the dependency instead of increasing throughput.
- Report saturation, timeout stage, active/idle capacity, and latency without
  credentials or query parameters.
- Retry only transient, replay-safe operations inside an attempt and age
  budget. State-changing calls need an idempotency contract first.
- A dependency outage removes readiness when the service cannot honor its
  contract; it must not cause an unbounded restart loop.

## See Also

- [async-http-client-reuse](async-http-client-reuse.md) - reuse clients and bound outbound HTTP calls
- [api-idempotency-key](api-idempotency-key.md) - make retried state changes replay-safe
- [api-health-probes](api-health-probes.md) - expose dependency readiness without restart amplification
- [obs-instrument-spans](obs-instrument-spans.md) - record dependency latency and failure stage
