# async-assert-send

> Assert that public futures and handles are `Send` so they can move across Tokio workers

## Why It Matters

A public `async fn` that holds `Rc` or a `!Send` guard across `.await` compiles until a caller writes `tokio::spawn`. The error then appears in *their* crate. The Microsoft Pragmatic Rust Guidelines require public futures — and most public handle types — to stay `Send`. A compile-time `assert_send` next to the entry point fails in *your* crate the moment a field or capture regresses. `static_assertions::assert_impl_all!` is the same check for named types.

## Bad

```rust
use std::rc::Rc;

pub async fn load(name: &str) {
    let _keep = Rc::new(name.to_string());
    async { let _ = name; }.await;
}

// Compiles here. `tokio::spawn(load("x"))` fails in the caller.
```

## Good

```rust
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

pub struct FileRead {
    _buf: Arc<[u8]>,
}

impl Future for FileRead {
    type Output = usize;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(0)
    }
}

const fn assert_send<T: Send>() {}
const _: () = assert_send::<FileRead>();

pub async fn load(name: Arc<str>) {
    let _ = name;
}

fn assert_future_send<T: Send>(_: &T) {}

fn main() {
    let fut = load(Arc::from("notes.txt"));
    assert_future_send(&fut);
}
```

## See Also

- [async-clone-before-await](async-clone-before-await.md) - drop `!Send` borrows before the future is spawned
- [own-arc-shared](own-arc-shared.md) - `Arc` is the `Send` sharing primitive
- [unsafe-send-sync-manual](unsafe-send-sync-manual.md) - do not paper over a `!Send` field with an unsafe impl
