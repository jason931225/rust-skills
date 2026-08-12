# api-no-wrapper-params

> Keep `Rc`, `Arc`, `Box`, and `RefCell` out of public function signatures unless sharing is the API

## Why It Matters

Smart pointers in a public signature leak an ownership scheme callers cannot change. Once two crates disagree about `Arc` versus `Rc`, or `Mutex` versus `RwLock`, the types no longer compose and the wrapper infects every downstream field. The Microsoft Pragmatic Rust Guidelines treat those wrappers as implementation details: accept and return `&T`, `&mut T`, or `T`, and hide any internal sharing behind the type.

## Bad

```rust
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

pub struct Config {
    pub name: String,
}

pub struct State {
    pub ready: bool,
}

// Callers must already live in this exact sharing scheme.
pub fn process_shared(data: Arc<Mutex<Config>>) -> Box<State> {
    let ready = data.lock().unwrap_or_else(|e| e.into_inner()).name.is_empty();
    Box::new(State { ready })
}

pub fn initialize(config: Rc<RefCell<Config>>) -> Arc<State> {
    let ready = config.borrow().name.is_empty();
    Arc::new(State { ready })
}
```

## Good

```rust
use std::sync::Arc;

pub struct Config {
    pub name: String,
}

pub struct State {
    ready: bool,
}

impl State {
    pub fn is_ready(&self) -> bool {
        self.ready
    }
}

// Borrow or take ownership; sharing stays inside the type if it is needed.
pub fn process_data(data: &Config) -> State {
    State {
        ready: !data.name.is_empty(),
    }
}

pub fn store_config(config: Config) -> State {
    State {
        ready: !config.name.is_empty(),
    }
}

// Sharing *is* the API: a handle type, not a raw Arc in every signature.
#[derive(Clone)]
pub struct SharedState {
    inner: Arc<State>,
}

impl SharedState {
    pub fn new(state: State) -> Self {
        Self {
            inner: Arc::new(state),
        }
    }

    pub fn is_ready(&self) -> bool {
        self.inner.is_ready()
    }
}
```

## See Also

- [own-arc-shared](own-arc-shared.md) - share with `Arc` behind a type, not in every parameter
- [api-sealed-trait](api-sealed-trait.md) - hide implementation choices that callers should not implement
- [anti-over-abstraction](anti-over-abstraction.md) - extra wrappers add type noise without adding capability
- [type-deref-coercion](type-deref-coercion.md) - do not paper over a leaked wrapper with `Deref`
- [api-service-clone](api-service-clone.md) - hide the Arc inside a Clone handle
- [api-std-types-boundary](api-std-types-boundary.md) - do not leak third-party types either
