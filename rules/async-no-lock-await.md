# async-no-lock-await

> Never hold a synchronous lock across `.await`; make async lock scope an explicit ownership contract

## Why It Matters

Holding a synchronous lock across `.await` can block an executor thread and
deadlock work needed by the suspended future. An async mutex does not block the
thread while waiting, and its guard is designed to cross `.await`, but it still
serializes every waiter for the whole suspended operation.

Default to extracting owned data, releasing the guard, and awaiting afterward.
Keep an async guard across `.await` only when exclusive ownership of the
resource is the actual protocol—for example, one framed connection whose
request and response must not interleave; bound that operation with a deadline,
keep unrelated state outside the guard, and prefer a dedicated owner task when
the protocol grows.

## Bad

```rust
use tokio::sync::Mutex;

async fn bad_update(state: &Mutex<State>) {
    let mut guard = state.lock().await;
    
    // BAD: Lock held across await!
    let data = fetch_from_network().await;
    
    guard.value = data;
}  // Lock finally released

// This can deadlock or starve other tasks
```

## Good

```rust
use tokio::sync::Mutex;

async fn good_update(state: &Mutex<State>) {
    // Fetch data BEFORE taking the lock
    let data = fetch_from_network().await;
    
    // Lock only for the quick update
    let mut guard = state.lock().await;
    guard.value = data;
}  // Lock released immediately

// Alternative: Clone data out, process, then update
async fn good_update_v2(state: &Mutex<State>) {
    // Extract what we need
    let id = {
        let guard = state.lock().await;
        guard.id.clone()
    };  // Lock released!
    
    // Do async work without lock
    let data = fetch_by_id(id).await;
    
    // Quick update
    state.lock().await.value = data;
}
```

## The Problem Visualized

```rust
// Task A:
let guard = mutex.lock().await;    // Acquires lock
expensive_io().await;              // Suspended, still holding lock!
// ... many milliseconds pass ...
drop(guard);                       // Finally releases

// Task B, C, D:
let guard = mutex.lock().await;    // All blocked waiting for A!
```

## Patterns for Extraction

```rust
use tokio::sync::Mutex;

// Pattern 1: Clone out, process, update
async fn pattern_clone(state: &Mutex<State>) {
    let config = state.lock().await.config.clone();
    let result = process_with_io(&config).await;
    state.lock().await.result = result;
}

// Pattern 2: Compute closure, apply
async fn pattern_closure(state: &Mutex<State>) {
    let update = compute_update().await;
    
    state.lock().await.apply(update);
}

// Pattern 3: Message passing
async fn pattern_message(
    state: &Mutex<State>,
    tx: mpsc::Sender<Update>,
) {
    let update = compute_update().await;
    tx.send(update).await.unwrap();
}

// Separate task handles updates
async fn state_manager(
    state: Arc<Mutex<State>>,
    mut rx: mpsc::Receiver<Update>,
) {
    while let Some(update) = rx.recv().await {
        state.lock().await.apply(update);
    }
}
```

## Releasing The Lock Can Be The Bug

Every pattern above splits one critical section into two, with an `.await`
between them. That is only correct when the second half does not depend on
state the first half observed. When it does, dropping the guard is not a fix —
it opens a check-then-act race that holding the lock would have prevented:

```rust
// Pattern 1 applied where the halves are *not* independent. Between the two
// locks, another task can change `version`, and this task then writes a
// result computed from a version that is no longer current — silently
// overwriting the other task's work.
async fn refresh(state: &Mutex<State>) {
    let version = state.lock().await.version;          // check
    let rendered = render(version).await;              // ...another task may bump `version` here
    state.lock().await.result = rendered;              // act, on a stale premise
}
```

Three honest resolutions, in preference order:

- **Make the halves independent.** Compute something that does not depend on
  the observed state, so the interleaving cannot matter. This is why the
  patterns above are written as clone-out / compute / apply.
- **Re-validate under the second lock.** Take the guard, confirm the premise
  still holds (`version` unchanged), and either commit or retry. This is a
  compare-and-swap in disguise and should be written as one.
- **Hold an async mutex across the whole transaction.** A
  `tokio::sync::Mutex` held across `.await` is legal and sometimes correct;
  the cost is that the critical section now includes I/O latency, which is a
  throughput decision to make deliberately rather than a rule violation.

The failure is invisible in a single-task test and reproduces only under
concurrency, so pin it with two tasks and an assertion on the torn state.

## Using RwLock

```rust
use tokio::sync::RwLock;

async fn read_heavy(state: &RwLock<State>) {
    // Multiple readers OK, but still don't hold across await
    let value = {
        let guard = state.read().await;
        guard.value.clone()
    };
    
    // Process without lock
    let result = process(value).await;
    
    // Write lock for update
    state.write().await.result = result;
}
```

## std::sync::Mutex vs tokio::sync::Mutex

```rust
use std::time::Duration;

struct State {
    counter: u64,
}

struct Connection;
struct Message;
struct Response;

impl Connection {
    async fn send_and_receive(
        &mut self,
        _message: Message,
    ) -> std::io::Result<Response> {
        Ok(Response)
    }
}

const REQUEST_DEADLINE: Duration = Duration::from_secs(2);

// A standard mutex can fit a short, non-awaiting critical section when
// contention and executor blocking have been measured and bounded.

fn quick_update(state: &std::sync::Mutex<State>) -> std::io::Result<()> {
    let mut state = state
        .lock()
        .map_err(|_| std::io::Error::other("state mutex poisoned"))?;
    state.counter = state
        .counter
        .checked_add(1)
        .ok_or_else(|| std::io::Error::other("counter overflow"))?;
    Ok(())
}

// tokio::sync::Mutex when exclusive resource ownership is the protocol:
async fn request(
    connection: &tokio::sync::Mutex<Connection>,
    message: Message,
) -> Result<Response, Box<dyn std::error::Error + Send + Sync>> {
    let mut connection = connection.lock().await;
    let response = tokio::time::timeout(
        REQUEST_DEADLINE,
        connection.send_and_receive(message),
    )
    .await??;
    Ok(response)
}
```

The second form intentionally serializes a connection whose wire protocol
permits one in-flight exchange. Prefer an owner task and bounded request queue
when that makes shutdown, backpressure, and failure handling clearer. It is not
a license to hold shared application-state locks during arbitrary network I/O.

## See Also

- [async-spawn-blocking](async-spawn-blocking.md) - Use spawn_blocking for CPU work
- [async-clone-before-await](async-clone-before-await.md) - Clone data before await
- [anti-lock-across-await](anti-lock-across-await.md) - Anti-pattern reference
