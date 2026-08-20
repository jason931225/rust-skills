# async-sync-core

> Keep business rules in sync functions that take I/O results as arguments; confine async to the outermost shell that fetches and orchestrates

## Why It Matters

Async colors every caller, so once an order function awaits an inventory client
the pricing arithmetic inside it can only be exercised through a runtime plus a
stub for every dependency the function reaches for. Failures then arrive as
mock-configuration problems rather than as pricing problems, and the same rule
cannot be reused from a batch job, a CLI, or a test fixture without dragging the
runtime along. Pushing the awaits outward — fetch first, then decide with the
fetched values — leaves the rules as ordinary functions a plain `#[test]` calls
directly.

## Bad

```rust
// Pure rules dragged into async because two steps in the middle need I/O.
async fn place_order(order: Order) -> Result<Invoice, OrderError> {
    if order.quantity == 0 {
        return Err(OrderError::EmptyOrder);          // pure
    }
    let stock = inventory.check(&order.sku).await?;  // I/O
    if stock < order.quantity {
        return Err(OrderError::OutOfStock);          // pure
    }
    let discount = pricing.lookup(order.customer).await?; // I/O
    Ok(Invoice::new(&order, discount))               // pure arithmetic
}

// Checking that a 25% discount yields 750 cents now requires a runtime and
// two fake services, neither of which has anything to do with the discount.
```

## Good

```rust
use std::collections::HashMap;

#[derive(Debug, PartialEq)]
pub enum OrderError {
    EmptyOrder,
    OutOfStock { sku: String, wanted: u32, available: u32 },
}

#[derive(Debug)]
pub struct Order {
    pub sku: String,
    pub quantity: u32,
    pub unit_price_cents: u64,
}

#[derive(Debug, PartialEq)]
pub struct Invoice {
    pub total_cents: u64,
}

/// The whole business rule: no `async`, no runtime, no client. The stock level
/// and the discount arrive as arguments, whatever went and fetched them.
pub fn price_order(
    order: &Order,
    available: u32,
    discount_percent: u64,
) -> Result<Invoice, OrderError> {
    if order.quantity == 0 {
        return Err(OrderError::EmptyOrder);
    }
    if available < order.quantity {
        return Err(OrderError::OutOfStock {
            sku: order.sku.clone(),
            wanted: order.quantity,
            available,
        });
    }
    let gross = u64::from(order.quantity) * order.unit_price_cents;
    Ok(Invoice { total_cents: gross - gross * discount_percent / 100 })
}

struct Catalog {
    on_hand: HashMap<String, u32>,
    discount_percent: u64,
}

impl Catalog {
    async fn stock(&self, sku: &str) -> u32 {
        tokio::task::yield_now().await; // stands in for a network round trip
        self.on_hand.get(sku).copied().unwrap_or(0)
    }

    async fn discount(&self, _sku: &str) -> u64 {
        tokio::task::yield_now().await;
        self.discount_percent
    }
}

/// The shell: fetch, fetch, decide. It carries no rule of its own, so swapping
/// HTTP for a cache changes this function and nothing below it.
async fn place_order(order: &Order, catalog: &Catalog) -> Result<Invoice, OrderError> {
    let available = catalog.stock(&order.sku).await;
    let discount = catalog.discount(&order.sku).await;
    price_order(order, available, discount)
}

fn main() {
    let order = Order { sku: "widget".to_owned(), quantity: 4, unit_price_cents: 250 };

    // Every rule is checked with no runtime and no fake service.
    assert_eq!(price_order(&order, 10, 25), Ok(Invoice { total_cents: 750 }));
    assert_eq!(
        price_order(&order, 3, 25),
        Err(OrderError::OutOfStock {
            sku: "widget".to_owned(),
            wanted: 4,
            available: 3,
        }),
        "the out-of-stock rule needs a stock number, not a stock client"
    );

    // One runtime-bound check remains, and it proves the wiring, not the rule.
    let catalog = Catalog {
        on_hand: HashMap::from([("widget".to_owned(), 10)]),
        discount_percent: 25,
    };
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let invoice = runtime.block_on(place_order(&order, &catalog)).expect("order priced");
    assert_eq!(invoice, Invoice { total_cents: 750 });
}
```

## Drawing The Async Boundary

- Apply the deletion test to every `async fn`: if removing the keyword would
  require nothing more than removing the keyword, the function never needed to
  be async and belongs in the core.
- Pass fetched values in, not the thing that fetches them. A core function that
  accepts a stock count works identically behind HTTP, gRPC, a cache, or a
  literal in a test; one that accepts a client does not.
- Async is earned where the concurrency *is* the rule — fan-out across several
  dependencies to pick a winner, streaming with backpressure, or a long-lived
  connection whose state transitions are driven by I/O events. Those stay async
  deliberately, with the reason recorded.
- Wrapping a large block of validation and formatting in `spawn_blocking` is a
  boundary symptom, not a fix: that code was never async, and a handler can call
  it directly. Reserve the blocking pool for work heavy enough to starve a
  worker.
- Library surfaces default to sync, because an async signature forces every
  consumer into a runtime, while a sync one lets the caller decide whether to
  offload. Offer an async convenience layer only as an addition.
- Keep the count of runtime-bound tests proportional to the number of distinct
  I/O sequences rather than to the number of business rules; a growing pile of
  `#[tokio::test]` unit tests means logic has drifted back into the shell.

## See Also

- [async-tokio-runtime](async-tokio-runtime.md) - the shell owns the runtime; the core never sees one
- [async-spawn-blocking](async-spawn-blocking.md) - a large blocking wrapper means the async boundary sits too deep
- [test-observable-coverage](test-observable-coverage.md) - business rules keep an oracle without mocks or a runtime
- [proj-lib-main-split](proj-lib-main-split.md) - the same split one layer out: thin entry point, testable logic
