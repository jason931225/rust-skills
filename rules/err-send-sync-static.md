# err-send-sync-static

> Make public error types `Send + Sync + 'static` so callers can move, wrap, and downcast them

## Why It Matters

An error that is not `Send` cannot cross a thread or task boundary, so a
caller cannot return it from `spawn`, store it in a shared handle, or box it
into `anyhow::Error`. One that borrows makes `Box<dyn Error + Send + Sync +
'static>` — the shape `io::Error::other` and most frameworks accept —
impossible. These bounds are a public API contract: adding them later is a
breaking change for nobody, but discovering their absence forces every caller
to stringify the error and lose the source chain.

## Bad

```rust
pub struct ParseError<'a> {
    // Borrowing the input pins the error to the buffer's lifetime, so it
    // cannot outlive the parse, cross a task, or be boxed as 'static
    offending: &'a str,
    source: Rc<dyn Error>,   // Rc is not Send, so neither is this error
}
```

## Good

```rust
use std::error::Error as StdError;
use std::fmt;

#[derive(Debug)]
pub struct StoreError {
    key: String,
    source: Option<Box<dyn StdError + Send + Sync + 'static>>,
}

impl StoreError {
    pub fn new(key: impl Into<String>) -> Self {
        Self { key: key.into(), source: None }
    }

    pub fn with_source(
        key: impl Into<String>,
        source: impl StdError + Send + Sync + 'static,
    ) -> Self {
        Self { key: key.into(), source: Some(Box::new(source)) }
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "store rejected key {}", self.key)
    }
}

impl StdError for StoreError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source.as_deref().map(|source| source as &(dyn StdError + 'static))
    }
}

fn assert_error_contract<E: StdError + Send + Sync + 'static>() {}

fn main() {
    // The bounds are the contract; assert them so a future field cannot
    // remove them silently.
    assert_error_contract::<StoreError>();

    let io = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
    let error = StoreError::with_source("orders/42", io);
    assert!(error.source().is_some(), "the cause survives boxing");

    // Because it is Send + Sync + 'static it fits the standard wrappers.
    let boxed: Box<dyn StdError + Send + Sync + 'static> = Box::new(error);
    let wrapped = std::io::Error::other(boxed);
    assert!(wrapped.get_ref().is_some());
}
```

## Key Points

- Own the data in the error: copy the offending fragment into a `String` or a
  bounded buffer instead of borrowing the input.
- Box the cause as `Box<dyn Error + Send + Sync + 'static>` so any nested
  error keeps the same guarantees.
- Assert the bounds in a test. They are easy to lose to a new field — an `Rc`,
  a `RefCell`, or a raw pointer removes them without a compile error at the
  definition site.
- `'static` here means the type borrows nothing, not that the value lives
  forever — an owned `String` field satisfies it.
- Enum errors get the same treatment; `thiserror` derives keep the bounds as
  long as every variant's payload has them.

## See Also

- [err-canonical-struct](err-canonical-struct.md) - keep the type opaque while preserving `source()`
- [err-source-chain](err-source-chain.md) - the chain these bounds keep intact
- [async-assert-send](async-assert-send.md) - the same assertion technique for futures
- [api-common-traits](api-common-traits.md) - which traits a public type owes its callers
