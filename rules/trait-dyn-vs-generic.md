# trait-dyn-vs-generic

> Prefer concrete types, then a closed enum, then narrow traits with generic parameters; hide `dyn` behind a crate-owned wrapper

## Why It Matters

It is easy to port an `IDatabase` interface into a Rust trait and then ask every caller for `Rc<dyn Database>` or `Box<dyn Database>`. That locks the crate out of constructs that are not object-safe, fights async (`async-fn-in-trait`), and leaks wrappers (`api-no-wrapper-params`). Microsoft Pragmatic Rust Guidelines (M-DI-HIERARCHY) escalate only as far as substitution actually requires: inherent methods on a concrete type first; a private enum for a small closed or test-only alternate set; narrow traits composed by subtraits when callers must supply implementations; generic parameters while they do not nest into `Foo<Bar<Baz>>`; `dyn Trait` last, and only behind a crate-owned wrapper — never a raw `Box` / `Arc` / `Rc` in the public API. Generics monomorphize and can inline; `dyn` is one vtable call and one code path. That cost model informs the last rung. It does not justify starting there.

## Bad

```rust
use std::path::PathBuf;
use std::rc::Rc;

struct Id;
struct Object;
struct MyDatabase;

// Naive C# `IDatabase` translation: one wide trait, used as a trait object.
trait Database {
    async fn update_config(&self, file: PathBuf);
    async fn store_object(&self, id: Id, obj: Object);
    async fn load_object(&self, id: Id) -> Object;
}

impl Database for MyDatabase {
    async fn update_config(&self, _file: PathBuf) {}
    async fn store_object(&self, _id: Id, _obj: Object) {}
    async fn load_object(&self, _id: Id) -> Object {
        Object
    }
}

// Intended to be used like this:
async fn start_service(_b: Rc<dyn Database>) {}
```

A public `Vec<Box<dyn Shape>>` or `Box<dyn Database>` parameter has the same shape: the wrapper and the trait object are the API. Heterogeneous storage may need `dyn` *inside* the crate; it is not a reason to publish the fat pointer.

## Good

Follow this ladder. Stop at the first rung that covers the substitutions you actually have.

### 1. Concrete type, inherent methods

If there is one implementation, there is no trait. Put the work on the type (`api-inherent-core`). Callers name `MyDatabase`, not `impl Database`.

### 2. Enum for a small closed or test-only set

If the other implementation exists only to provide a sans-I/O or test double, keep a private enum and dispatch there (`test-mock-traits`). Do not open a trait so a test can inject a mock.

```rust
enum DataAccess {
    MyDatabase(MyDatabase),
    Mock(mock::MockCtrl),
}

struct MyDatabase;
mod mock {
    pub struct MockCtrl;
}

async fn read_database(_x: &DataAccess) {}
```

### 3. Narrow traits, composed by subtraits

When users are expected to provide implementations, add one or more *narrow* traits on top of the inherent methods. `StoreObject` and `LoadObject` beat a single `Database` kitchen sink. If a combined bound is eventually needed, make it a subtrait.

```rust
trait StoreObject {
    async fn store_object(&self, id: Id, obj: Object);
}

trait LoadObject {
    async fn load_object(&self, id: Id) -> Object;
}

trait DataAccess: StoreObject + LoadObject {}

struct Id;
struct Object;
```

### 4. Generic parameters for open substitution

Code that works with those traits should take generic type parameters (or `impl Trait`) while the type remains local and readable. Use the most specific trait, not the umbrella, at each call site.

```rust
// Good, generic does not have infectious impact, uses only most specific trait
async fn read_database(x: impl LoadObject) {
    let _ = x;
}

// Acceptable, unless further nesting makes this excessive.
struct MyService<T: DataAccess> {
    db: T,
}

trait LoadObject {}
trait StoreObject {}
trait DataAccess: StoreObject + LoadObject {}
```

A service type that would have to be named as `Service<Backend<Store>>` has gone too far (`anti-over-abstraction`). That is the point to stop adding type parameters, not the point to start sprinkling `dyn` into every signature.

### 5. `dyn` behind a crate-owned wrapper

When generic layers start leaking through several public types, runtime dispatch becomes a reasonable trade. Even then, do not publish `Box<dyn Trait>` or `Arc<dyn Trait>`. Hide the fat pointer in a type you own so you can change the storage, implement the trait for the wrapper, and keep using ordinary generic functions.

```rust
use std::sync::Arc;

trait DataAccess {
    fn foo(&self);
}

// This allows you to expand or change `DynamicDataAccess` later. You can also
// implement `DataAccess` for `DynamicDataAccess` if needed, and use it with
// regular generic functions.
struct DynamicDataAccess(Arc<dyn DataAccess>);

impl DynamicDataAccess {
    fn new<T: DataAccess + 'static>(db: T) -> Self {
        Self(Arc::new(db))
    }
}

struct MyService {
    db: DynamicDataAccess,
}
```

The wrapper combines with the enum rung when a crate needs a native impl, a test double, *and* an escape hatch for unknown runtime types:

```rust
enum DataAccess {
    MyDatabase(MyDatabase),
    Mock(mock::MockCtrl),
    Dynamic(DynamicDataAccess),
}

async fn read_database(_x: &DataAccess) {}

struct MyDatabase;
struct DynamicDataAccess;
mod mock {
    pub struct MockCtrl;
}
```

The trait still has to be dyn-compatible if this rung is in play (`trait-object-safety`). Native `async fn` in the trait is not; name the future or keep this rung off the async surface (`async-fn-in-trait`).

## Escalation Ladder

| Situation | Choose |
|---|---|
| One implementation | Concrete type, inherent methods |
| Small closed set, or a test-only / sans-I/O alternate | Private enum |
| Callers must supply behavior | Narrow traits on top of inherent methods; compose with subtraits |
| Open substitution, no nesting problem | Generic parameter / `impl Trait` |
| Generics nest or the set is heterogeneous at runtime | `dyn Trait` inside a crate-owned wrapper |
| Public `Box<dyn …>`, `Arc<dyn …>`, `Rc<dyn …>` | Never — that wrapper *is* the leak |

`anti-type-erasure` still prefers `impl Trait` over `Box<dyn Trait>` when a single concrete type would do. This rule adds the public-API constraint: when `dyn` is genuinely required, the crate owns the handle. Hot-path inlining is a reason to stay generic on rung 4, not a reason to skip rungs 1–3.

## See Also

- [api-inherent-core](api-inherent-core.md) - the first rung: essential methods live on the type
- [test-mock-traits](test-mock-traits.md) - the enum rung for clocks, I/O, and other syscalls
- [api-no-wrapper-params](api-no-wrapper-params.md) - do not publish `Box` / `Arc` / `Rc` as the API
- [anti-type-erasure](anti-type-erasure.md) - do not erase a type you already know
- [anti-over-abstraction](anti-over-abstraction.md) - nesting generics is the signal to wrap, not to add another parameter
- [type-generic-bounds](type-generic-bounds.md) - keep bounds on the functions that need them
- [trait-object-safety](trait-object-safety.md) - a trait used on the `dyn` rung must stay dyn-compatible
- [async-fn-in-trait](async-fn-in-trait.md) - native async traits are not dyn-compatible
