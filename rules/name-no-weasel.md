# name-no-weasel

> Drop empty role words like `Service`, `Manager`, and `Factory` from type names

## Why It Matters

Every type manages something; putting `Manager` in the name does not tell a reader what the type *does*. Following Microsoft Pragmatic Rust Guidelines (M-WEASEL-WORDS), drop empty role words like `Service`, `Manager`, and `Factory`: replace them with the noun for the work (`Invoices`, `InvoiceDispatcher`) and use `Builder` when the type exists to construct another value. Shorter, specific names also survive grepping and rustdoc search.

## Bad

```rust
// Role word says nothing about invoices.
pub struct InvoiceService {
    pub count: usize,
}

// Every type "manages" something; the noun is missing.
pub struct LedgerManager {
    pub live: usize,
}

pub struct ReceiptFactory;

impl ReceiptFactory {
    pub fn create(&self) -> u64 {
        1
    }
}
```

## Good

```rust
pub struct Invoices {
    pub count: usize,
}

pub struct InvoiceDispatcher {
    pub live: usize,
}

pub struct ReceiptBuilder {
    seed: u64,
}

impl ReceiptBuilder {
    pub fn new() -> Self {
        Self { seed: 1 }
    }

    pub fn build(self) -> u64 {
        self.seed
    }
}

fn main() {
    let _ = ReceiptBuilder::new().build();
}
```

## See Also

- [name-types-camel](name-types-camel.md) - casing still follows UpperCamelCase after the extra words come out
- [name-crate-no-rs](name-crate-no-rs.md) - the same rule at crate scope: do not pad a name with a redundant suffix
- [api-builder-pattern](api-builder-pattern.md) - the idiomatic name for a repeated constructor is `FooBuilder`
