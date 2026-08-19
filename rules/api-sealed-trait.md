# api-sealed-trait

> Use sealed traits to prevent external implementations while allowing use

## Why It Matters

Public traits can be implemented by anyone, which may be undesirable when you need to guarantee behavior or add methods in future versions. A sealed trait can be used by external code but not implemented by it, giving you control over implementations while maintaining a usable API.

## Bad

```rust
// Anyone can implement this trait
pub trait DatabaseDriver {
    fn connect(&self, url: &str) -> Connection;
    fn execute(&self, query: &str) -> Result<Rows, Error>;
}

// External crate implements it incorrectly
impl DatabaseDriver for MyBadDriver {
    fn connect(&self, url: &str) -> Connection {
        // Buggy implementation that doesn't handle errors
        unsafe { force_connect(url) }
    }
}

// Later, you want to add a required method - BREAKING CHANGE
pub trait DatabaseDriver {
    fn connect(&self, url: &str) -> Connection;
    fn execute(&self, query: &str) -> Result<Rows, Error>;
    fn transaction(&self) -> Transaction;  // External impls now broken!
}
```

## Good

```rust
// Create a private module with a private trait
mod private {
    pub trait Sealed {}
}

// Public trait requires the private trait
/// This trait is sealed and cannot be implemented outside this crate.
pub trait DatabaseDriver: private::Sealed {
    fn connect(&self, url: &str) -> Connection;
    fn execute(&self, query: &str) -> Result<Rows, Error>;
}

// Only your crate can implement Sealed, thus DatabaseDriver
pub struct PostgresDriver;
impl private::Sealed for PostgresDriver {}
impl DatabaseDriver for PostgresDriver {
    fn connect(&self, url: &str) -> Connection { ... }
    fn execute(&self, query: &str) -> Result<Rows, Error> { ... }
}

pub struct MySqlDriver;
impl private::Sealed for MySqlDriver {}
impl DatabaseDriver for MySqlDriver {
    fn connect(&self, url: &str) -> Connection { ... }
    fn execute(&self, query: &str) -> Result<Rows, Error> { ... }
}

// External crate cannot implement - private::Sealed is not accessible
// impl DatabaseDriver for ExternalDriver { }  // Error!

// But external code CAN use the trait
fn use_driver(driver: &impl DatabaseDriver) {
    let conn = driver.connect("postgres://localhost");
}
```

## Benefits of Sealing

```rust
// 1. Add methods without breaking changes
pub trait Format: private::Sealed {
    fn format(&self) -> String;
    
    // Added later - not breaking because no external impls exist
    fn format_pretty(&self) -> String {
        self.format()  // Default implementation
    }
}

// 2. Guarantee invariants
pub trait SafeBuffer: private::Sealed {
    // You control all implementations, so you know they're all correct
    fn get(&self, index: usize) -> Option<&u8>;
}

// 3. Use as marker traits
pub trait ValidConfig: private::Sealed {}
// Only validated configs implement this
```

## Partially Sealed

Sealing is all-or-nothing *per trait*: the supertrait bound makes every
external `impl Plugin for T` impossible, so no method of a sealed trait can be
left open for callers to override. Splitting the surface into two traits is
what gives a partially open API.

```rust
mod private {
    pub trait SealedCore {}
}

/// Sealed: only this crate implements it, and methods may be added without a
/// major version bump.
pub trait Plugin: private::SealedCore {
    fn initialize(&self);
    fn shutdown(&self);
}

/// Unsealed companion: callers may implement or override this.
pub trait PluginName {
    fn name(&self) -> &str {
        "unnamed"
    }
}

struct Builtin;
impl private::SealedCore for Builtin {}
impl Plugin for Builtin {
    fn initialize(&self) {}
    fn shutdown(&self) {}
}
// A downstream crate can do this, because `PluginName` is not sealed.
impl PluginName for Builtin {
    fn name(&self) -> &str {
        "builtin"
    }
}

fn main() {
    assert_eq!(Builtin.name(), "builtin");
}
```

## When to Seal

| Seal When | Don't Seal When |
|-----------|-----------------|
| API stability is critical | You want extension points |
| Implementation correctness is hard | Users need custom implementations |
| You'll add methods later | Trait is simple and stable |
| Safety invariants required | Standard patterns (Iterator, etc.) |

## See Also

- [api-non-exhaustive](./api-non-exhaustive.md) - Related pattern for enums/structs
- [api-extension-trait](./api-extension-trait.md) - Adding methods to external types
- [api-typestate](./api-typestate.md) - Compile-time state guarantees
