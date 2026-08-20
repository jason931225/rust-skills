# api-typed-response

> Build an outbound payload by serializing a typed value, not by assembling an untyped tree in the handler

## Why It Matters

The library's boundary rules all face inward: parse what arrives, reject what
is malformed. The producing side has the mirror obligation and a worse blast
radius — a handler that builds a `json!` tree by hand can omit a required
field, misspell a key, or emit a number where the schema says string, and ship
that to every client at once. A typed value makes the compiler check what the
contract requires: a missing field is a compile error, and a renamed key
changes in one place.

## Bad

```rust
async fn get_order(id: OrderId) -> Json<Value> {
    let order = store.load(id).await?;
    // Nothing checks these keys against the documented schema; a typo or an
    // omitted field ships to every consumer
    Json(json!({
        "orderId": order.id,
        "total": order.total.to_string(),
    }))
}
```

## Good

```rust
/// The response schema, expressed once as a type. A missing field will not
/// compile, and the wire names live beside the fields they rename.
#[derive(Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderResponse {
    pub order_id: u64,
    pub total_cents: i64,
    /// Absent from the payload rather than emitted as null.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancelled_at: Option<String>,
}

pub struct Order {
    pub id: u64,
    pub total_cents: i64,
    pub cancelled_at: Option<String>,
}

impl From<Order> for OrderResponse {
    fn from(order: Order) -> Self {
        Self {
            order_id: order.id,
            total_cents: order.total_cents,
            cancelled_at: order.cancelled_at,
        }
    }
}

fn main() {
    let response = OrderResponse::from(Order { id: 7, total_cents: 1250, cancelled_at: None });
    let body = serde_json::to_string(&response).expect("serializes");

    assert_eq!(body, r#"{"orderId":7,"totalCents":1250}"#);
    // The optional field is absent, not null, and the names are the contract's.
    assert!(!body.contains("cancelled"));
    assert!(!body.contains("order_id"));
}
```

## Response Type Boundaries

- Keep the response type separate from the domain type, so an internal field
  cannot leak by being added to a struct that happens to be serialized.
- Derive the conversion from domain to response in one place; handlers should
  build the typed value, never the payload.
- Snapshot-test the serialized form. The type checks presence; only a snapshot
  catches a rename that compiles.
- `skip_serializing_if` states whether absent and null mean the same thing to
  consumers — decide it rather than inheriting it.
- Errors follow the same rule and have their own contract.

## See Also

- [api-error-schema](api-error-schema.md) - the failing half of the same surface
- [api-extract-or-reject](api-extract-or-reject.md) - the inbound mirror of this rule
- [serde-rename-all](serde-rename-all.md) - matching the external naming convention
- [test-snapshot-testing](test-snapshot-testing.md) - catching a rename the compiler accepts
