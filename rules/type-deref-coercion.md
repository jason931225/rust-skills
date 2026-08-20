# type-deref-coercion

> Implement `Deref`/`DerefMut` only for smart-pointer and transparent wrapper types

## Why It Matters

`Deref` coercions are what make `Box<T>`, `Arc<T>`, `String`, and `Vec<T>` ergonomic — they let the inner type's methods surface through the wrapper transparently. The Rust API Guidelines (C-DEREF) specify this usage precisely: implement `Deref<Target = T>` when your type *is* a smart pointer or a transparent container for `T`. Using it as an OOP-style inheritance mechanism pollutes method resolution, confuses readers, and makes refactoring hazardous because adding methods to `T` silently affects every wrapper that `Deref`s to it.

## Bad

```rust
struct User {
    name: String,
    email: String,
}

struct AdminUser(User);

// Anti-pattern: using Deref to "inherit" User methods on AdminUser
impl std::ops::Deref for AdminUser {
    type Target = User;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// Now AdminUser silently exposes all User fields/methods —
// callers can't tell what AdminUser owns vs. inherits.
fn greet(admin: &AdminUser) {
    println!("hello, {}", admin.name); // surprising implicit deref
}
```

## Good

```rust
// Smart-pointer/transparent wrapper: correct use of Deref
struct MyBox<T>(T);

impl<T> std::ops::Deref for MyBox<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> std::ops::DerefMut for MyBox<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// Domain types: expose only the API you intend, explicitly
struct User {
    pub name: String,
    pub email: String,
}

struct AdminUser(User);

impl AdminUser {
    pub fn name(&self) -> &str {
        &self.0.name
    }

    pub fn email(&self) -> &str {
        &self.0.email
    }

    pub fn can_delete_users(&self) -> bool {
        true
    }
}

fn greet(admin: &AdminUser) {
    println!("hello, {}", admin.name()); // explicit, readable
}
```

## Legitimate Uses

- `Box<T>`, `Rc<T>`, `Arc<T>` — pointer indirection
- `String` → `str`, `Vec<T>` → `[T]` — owned-to-borrowed transparent containers
- `MutexGuard<T>` → `T` — RAII guards that provide temporary access
- Newtype wrappers that are *genuinely transparent* — the wrapper adds a name
  or a marker but no invariant the inner type could violate

## Not For Invariant-Bearing Newtypes

A newtype whose purpose is "a `T` that additionally guarantees X" is the case
where `Deref` is most dangerous, not a legitimate use of it. `DerefMut` is an
outright hole: it hands out `&mut T`, so a direct assignment bypasses the
constructor that established the invariant.

```rust
use std::num::NonZeroU16;

pub struct Port(NonZeroU16);

// With this impl, `*port = 0` would compile and void the invariant the
// constructor exists to establish — the validation becomes advisory.
// impl std::ops::DerefMut for Port {
//     fn deref_mut(&mut self) -> &mut u16 { /* ... */ }
// }

impl Port {
    pub fn new(value: u16) -> Option<Self> {
        NonZeroU16::new(value).map(Port)
    }

    /// Read-only access, so the invariant cannot be assigned away.
    pub fn get(&self) -> u16 {
        self.0.get()
    }
}

fn main() {
    let port = Port::new(8080).expect("8080 is non-zero");
    assert_eq!(port.get(), 8080);
    assert!(Port::new(0).is_none(), "the constructor is the only way in");
}
```

Plain `Deref` is weaker but still wrong here for a different reason: it
surfaces the inner type's entire API through the wrapper. An `Email(String)`
that derefs to `str` exposes `split_at`, `trim`, and every future `str`
method, none of which preserve "this is a valid email" — and the set grows
with each release of the standard library, so the wrapper's surface changes
without its own code changing.

Expose what the invariant permits instead: an inherent accessor, `AsRef<T>`
for read-only borrowing, or explicit delegation of the few methods that make
sense. Each is more typing at the call site and each keeps the newtype's
guarantee intact.

## See Also

- [api-newtype-safety](api-newtype-safety.md) - newtypes for type-safe distinctions without inheritance
- [type-newtype-ids](type-newtype-ids.md) - wrapping IDs in newtypes
- [own-borrow-over-clone](own-borrow-over-clone.md) - prefer `&T` borrowing over `.clone()`
