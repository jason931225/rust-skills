# async-assert-send

> Assert that public futures and handles are `Send` so they can move across Tokio workers

## Why It Matters

A public `async fn` that holds `Rc` or a `!Send` guard across `.await` may
compile until a caller uses `tokio::spawn`, moving the error into their crate.
Public futures and handles intended to cross workers should stay `Send`, with a
compile-time `require_send` beside each main entry point. Do not assert every
helper mechanically. A `!Send` temporary is fine when it is created, used, and
dropped before any `.await`.

## Bad

```rust
use std::rc::Rc;

pub async fn fetch_blob(name: &str) {
    let _keep = Rc::new(name.to_string());
    async { let _ = name; }.await;
}

// Compiles here. `tokio::spawn(fetch_blob("x"))` fails in the caller.
```

## Good

```rust
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

pub struct BlobRead {
    _buf: Arc<[u8]>,
}

impl Future for BlobRead {
    type Output = usize;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(0)
    }
}

const fn require_send<T: Send>() {}
const _: () = require_send::<BlobRead>();

pub async fn fetch_blob(name: Arc<str>) {
    let length = {
        let local = std::rc::Rc::new(name.len());
        *local
    }; // `!Send` state leaves scope before the await below.
    std::future::ready(()).await;
    let _ = (name, length);
}

fn require_future_send<T: Send>(_: &T) {}

fn main() {
    let fut = fetch_blob(Arc::from("notes.txt"));
    require_future_send(&fut);
}
```

## Generic Parameters Carry The Bound The Assertion Cannot

The compile-time assertion above pins a concrete future. It cannot be written
for a generic one — there is no single type to name — so a generic API that
hands its parameter to another worker has to declare the bound on its own
signature instead.

An `async fn` generic over `T` compiles without `Send` on `T`, because the
obligation only appears once a caller spawns it. That is where the diagnostic
appears too:

```text
error[E0277]: `Rc<u8>` cannot be sent between threads safely
   --> src/main.rs:7:32
note: required because it's used within this `async` fn body
   --> src/main.rs:2:40
note: required by a bound in `tokio::spawn`
   --> .../tokio-1.53.1/src/task/spawn.rs:176:21
```

The caller is told their value is wrong by way of a line inside somebody else's
function body and a line inside tokio. Declaring the bound moves it to the
signature the caller actually wrote against:

```text
error[E0277]: `Rc<u8>` cannot be sent between threads safely
   = help: the trait `Send` is not implemented for `Rc<u8>`
note: required by a bound in `store`
```

```rust
use std::fmt::Debug;

// The bound is part of the contract, not an accident of the body.
pub async fn store<T: Debug + Send + 'static>(value: T) {
    let _ = value;
}
```

This is one of the places where adding a bound that is not strictly required to
compile is correct: the bound documents that the value will cross a worker,
and it fails at the boundary where it can be fixed. Add it when the parameter
is spawned, sent to a task, or stored somewhere a task will reach; leave it off
when the value never leaves the caller's thread, since `Send + 'static` on a
parameter that stays local narrows the API for nothing.

## See Also

- [async-clone-before-await](async-clone-before-await.md) - drop `!Send` borrows before the future is spawned
- [own-arc-shared](own-arc-shared.md) - `Arc<T>` can cross threads only when `T` permits it
- [unsafe-send-sync-manual](unsafe-send-sync-manual.md) - do not paper over a `!Send` field with an unsafe impl
