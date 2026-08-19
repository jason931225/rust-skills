# async-cancellation-token

> Use `CancellationToken` for graceful shutdown and task cancellation

## Why It Matters

Dropping a `JoinHandle` doesn't cancel the task—it just detaches it. For graceful shutdown, you need explicit cancellation. `tokio_util::sync::CancellationToken` provides a cooperative cancellation mechanism that tasks can check and respond to, enabling clean resource cleanup.

## Bad

```rust
// Dropping handle doesn't stop the task
let handle = tokio::spawn(async {
    loop {
        do_work().await;
    }
});

drop(handle);  // Task continues running in background!

// Using bool flag - not async-aware
let running = Arc::new(AtomicBool::new(true));

tokio::spawn({
    let running = running.clone();
    async move {
        while running.load(Ordering::Relaxed) {
            do_work().await;  // Can't wake up if blocked here
        }
    }
});

running.store(false, Ordering::Relaxed);
// Task won't stop until current do_work() completes
```

## Good

```rust
use tokio_util::sync::CancellationToken;

let token = CancellationToken::new();

let handle = tokio::spawn({
    let token = token.clone();
    async move {
        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    println!("Shutting down gracefully");
                    cleanup().await;
                    break;
                }
                _ = do_work() => {
                    // Work completed
                }
            }
        }
    }
});

// Later: trigger cancellation
token.cancel();
handle.await?;  // Task completes cleanly
```

## CancellationToken API

```rust
use tokio_util::sync::CancellationToken;

// Create token
let token = CancellationToken::new();

// Clone for sharing (cheap Arc-based clone)
let token2 = token.clone();

// Check if cancelled (non-blocking)
if token.is_cancelled() {
    return;
}

// Wait for cancellation (async)
token.cancelled().await;

// Trigger cancellation
token.cancel();

// Child tokens - cancelled when parent is cancelled
let child = token.child_token();
```

## Hierarchical Cancellation

```rust
async fn run_server(shutdown: CancellationToken) {
    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    
    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                println!("Server shutting down");
                break;
            }
            result = listener.accept() => {
                let (socket, _) = result?;
                // Each connection gets child token
                let conn_token = shutdown.child_token();
                tokio::spawn(handle_connection(socket, conn_token));
            }
        }
    }
    
    // Child tokens auto-cancelled when we exit
}

async fn handle_connection(socket: TcpStream, token: CancellationToken) {
    loop {
        tokio::select! {
            _ = token.cancelled() => {
                // Connection cleanup
                break;
            }
            data = socket.read() => {
                // Handle data
            }
        }
    }
}
```

## Graceful Shutdown Pattern

A container runtime or service manager asks a process to stop with `SIGTERM`,
not `SIGINT`, and follows it with `SIGKILL` after a grace period. Listening
only for Ctrl+C means production shutdowns are always kills: connections are
severed mid-request and in-flight work is lost.

```rust
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

/// Drain budget, deliberately shorter than the platform's grace period so the
/// process exits on its own terms instead of being killed mid-drain.
const DRAIN_BUDGET: Duration = Duration::from_secs(25);
/// Time for load balancers to observe the failing readiness probe.
const READINESS_PROPAGATION: Duration = Duration::from_secs(5);

/// Resolves when the process is asked to stop. `SIGTERM` is what orchestrators
/// send; `SIGINT` covers an interactive run.
async fn shutdown_signal() -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let mut terminate = signal(SignalKind::terminate())?;
        let mut interrupt = signal(SignalKind::interrupt())?;
        tokio::select! {
            _ = terminate.recv() => {}
            _ = interrupt.recv() => {}
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await
    }
}

async fn serve(_shutdown: CancellationToken) {}
async fn worker(_shutdown: CancellationToken) {}

async fn run(ready: Arc<AtomicBool>) -> std::io::Result<()> {
    let shutdown = CancellationToken::new();
    let mut tasks = JoinSet::new();
    tasks.spawn(serve(shutdown.child_token()));
    tasks.spawn(worker(shutdown.child_token()));

    shutdown_signal().await?;
    tracing::info!(event = "shutdown.started", "draining");

    // 1. Fail readiness first, so routing stops sending new requests while the
    //    process is still able to finish the ones it already has.
    ready.store(false, Ordering::SeqCst);
    tokio::time::sleep(READINESS_PROPAGATION).await;

    // 2. Then cancel: accept loops stop taking work and in-flight tasks observe
    //    the token at their next cancellation point.
    shutdown.cancel();

    // 3. Drain within the budget; abort whatever is left rather than hanging
    //    until the runtime sends SIGKILL.
    let drained = tokio::time::timeout(DRAIN_BUDGET, async {
        while tasks.join_next().await.is_some() {}
    })
    .await;
    if drained.is_err() {
        tracing::warn!(event = "shutdown.drain_timeout", "aborting unfinished tasks");
        tasks.shutdown().await;
    }
    Ok(())
}
```

Order matters: cancelling before readiness fails sheds requests that were
already accepted, and exiting before the drain budget elapses truncates work
that would have finished. `SIGKILL` cannot be handled at all, so the budget
here must stay below the platform's grace period — 30 seconds by default in
Kubernetes, configurable per workload.

## DropGuard Pattern

```rust
use tokio_util::sync::CancellationToken;

// Auto-cancel on drop
let token = CancellationToken::new();
let guard = token.clone().drop_guard();

tokio::spawn({
    let token = token.clone();
    async move {
        token.cancelled().await;
        println!("Cancelled!");
    }
});

drop(guard);  // Automatically calls token.cancel()
```

## See Also

- [async-joinset-structured](./async-joinset-structured.md) - Managing multiple tasks
- [async-select-racing](./async-select-racing.md) - select! for cancellation
- [async-tokio-runtime](./async-tokio-runtime.md) - Runtime shutdown
- [api-health-probes](./api-health-probes.md) - Readiness is what stops new work arriving
