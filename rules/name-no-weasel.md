# name-no-weasel

> Drop empty role words like `Service`, `Manager`, and `Factory` from type names

## Why It Matters

Every type manages something; putting `Manager` in the name does not tell a reader what the type *does*. The Microsoft Pragmatic Rust Guidelines treat `Service`, `Manager`, and `Factory` as weasel words: replace them with the noun for the work (`Bookings`, `BookingDispatcher`) and use `Builder` when the type exists to construct another value. Shorter, specific names also survive grepping and rustdoc search.

## Bad

```rust
pub struct BookingService {
    pub count: usize,
}

pub struct SessionManager {
    pub live: usize,
}

pub struct TicketFactory;

impl TicketFactory {
    pub fn create(&self) -> u64 {
        1
    }
}
```

## Good

```rust
pub struct Bookings {
    pub count: usize,
}

pub struct BookingDispatcher {
    pub live: usize,
}

pub struct TicketBuilder {
    seed: u64,
}

impl TicketBuilder {
    pub fn new() -> Self {
        Self { seed: 1 }
    }

    pub fn build(self) -> u64 {
        self.seed
    }
}

fn main() {
    let _ = TicketBuilder::new().build();
}
```

## See Also

- [name-types-camel](name-types-camel.md) - casing still follows UpperCamelCase after the extra words come out
- [name-crate-no-rs](name-crate-no-rs.md) - the same rule at crate scope: do not pad a name with a redundant suffix
- [api-builder-pattern](api-builder-pattern.md) - the idiomatic name for a repeated constructor is `FooBuilder`
