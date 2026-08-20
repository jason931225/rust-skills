# err-context-chain

> Add context with `.context()` or `.with_context()`

## Why It Matters

Raw errors often lack information about what operation failed. Adding context creates an error chain that tells the full story: what you were trying to do, and why it failed.

## Bad

```rust
// Raw error - no context
fn load_user(id: u64) -> Result<User, Error> {
    let path = format!("users/{}.json", id);
    let content = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&content)?)
}

// Error message: "No such file or directory (os error 2)"
// Which file? What were we doing?
```

## Good

```rust
use anyhow::{Context, Result};

fn load_user(id: u64) -> Result<User> {
    let path = format!("users/{}.json", id);
    
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read user file: {}", path))?;
    
    let user: User = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse user {} JSON", id))?;
    
    Ok(user)
}

// Error: "failed to parse user 42 JSON"
// Caused by: "expected ':' at line 5 column 12"
```

## context() vs with_context()

```rust
// context() - static string (slight allocation)
fs::read_to_string(path)
    .context("failed to read config")?;

// with_context() - lazy evaluation (only allocates on error)
fs::read_to_string(path)
    .with_context(|| format!("failed to read {}", path))?;

// Use with_context() when:
// - Message includes runtime data (format!)
// - Computing the message is expensive
// - Error path is cold (most of the time)
```

## Building Context Chains

```rust
fn process_order(order_id: u64) -> Result<()> {
    let order = fetch_order(order_id)
        .with_context(|| format!("failed to fetch order {}", order_id))?;
    
    let user = load_user(order.user_id)
        .with_context(|| format!("failed to load user for order {}", order_id))?;
    
    let payment = process_payment(&order, &user)
        .context("payment processing failed")?;
    
    ship_order(&order, &payment)
        .context("shipping failed")?;
    
    Ok(())
}

// Full error chain:
// "shipping failed"
// Caused by: "carrier API returned 503"
// Caused by: "connection refused"
```

## Nested Results From Tasks And Timeouts

`JoinHandle::await` and `tokio::time::timeout` both yield
`Result<Result<T, E>, Outer>` — the outer error is a `JoinError` or an
`Elapsed`, and the inner one is whatever the work actually returned. Context
attached at that boundary lands on the *outer* result only, so a completed
inner failure rides through inside `Ok` and never receives it:

```rust
// Reads as if it labels the worker's failure. It does not: `.context()`
// applies to the JoinError, and the inner `E` passes through untouched.
// let value = handle.await.context("spawned worker failed")??;

// This one is subtler than it looks: `map_err` here applies to the OUTER
// `Elapsed` only, so an inner `Err(e)` still propagates as `e`. The bug is
// not mislabelling — it is that the outer and inner failures now flow
// through the same `?` with only one of them carrying any context.
```

Handle the two separately: attach context to the work *inside* the spawned
future or immediately after unwrapping the inner `Result`, and map the outer
error on its own. Otherwise a task that panicked and a task that returned an
error become indistinguishable, and a timed-out call reports `Timeout` for a
request that actually failed for some other reason.

- Attach the inner context where the inner error is produced — inside the
  spawned future — so it survives regardless of how the join is written.
- Map `JoinError` and `Elapsed` explicitly, and say which one occurred; a
  panicked task and a cancelled one are different operational events.
- Assert the shape in a test: a failing inner operation's source chain should
  contain the inner context, and a timed-out `Ok(Err(e))` should still surface
  `e` rather than a fabricated timeout.

## Displaying Error Chains

```rust
fn main() {
    if let Err(e) = run() {
        // Just top-level message
        eprintln!("Error: {}", e);
        
        // Full chain with alternate format
        eprintln!("Error: {:#}", e);
        
        // Debug format (includes backtrace if enabled)
        eprintln!("Error: {:?}", e);
        
        // Iterate through chain
        for (i, cause) in e.chain().enumerate() {
            eprintln!("  {}: {}", i, cause);
        }
    }
}
```

## With thiserror

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("failed to load config from {path}")]
    ConfigLoad {
        path: String,
        #[source]
        cause: std::io::Error,
    },
    
    #[error("failed to connect to database")]
    Database {
        #[source]
        cause: sqlx::Error,
    },
}

// Usage
fn load_config(path: &str) -> Result<Config, AppError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| AppError::ConfigLoad {
            path: path.to_string(),
            cause: e,
        })?;
    // ...
}
```

## See Also

- [err-anyhow-app](err-anyhow-app.md) - Use anyhow for applications
- [err-source-chain](err-source-chain.md) - Use #[source] to chain errors
- [err-question-mark](err-question-mark.md) - Use ? for propagation
